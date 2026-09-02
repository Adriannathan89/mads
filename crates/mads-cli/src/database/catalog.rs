use std::collections::{BTreeMap, BTreeSet};

use mads::diesel;
use mads::{
    Database,
    diesel::{
        QueryableByName, RunQueryDsl,
        pg::PgConnection,
        result::QueryResult,
        sql_query,
        sql_types::{Array, Bool, Integer, Nullable, Text},
    },
};

use super::schema::{ColumnSchema, PgType, QualifiedTableName, TableSchema};
use crate::diagnostic::{CliError, MADS211};

/// A normalized snapshot of supported PostgreSQL table shape and unsupported evidence.
#[derive(Debug)]
pub(crate) struct LiveSchema {
    tables: BTreeMap<QualifiedTableName, TableSchema>,
    unsupported: Vec<UnsupportedObject>,
    foreign_keys: Vec<ForeignKeyDependency>,
}

/// A PostgreSQL object category that schema-diff generation does not synthesize.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum UnsupportedKind {
    /// A column default.
    Default,
    /// A non-primary index.
    Index,
    /// A check constraint.
    Check,
    /// A non-internal trigger.
    Trigger,
    /// A foreign-key constraint.
    ForeignKey,
}

/// Evidence for one unsupported PostgreSQL object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UnsupportedObject {
    pub(crate) kind: UnsupportedKind,
    pub(crate) table: QualifiedTableName,
    pub(crate) name: String,
    pub(crate) columns: Vec<String>,
}

/// One live foreign-key dependency used to order or reject later table changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ForeignKeyDependency {
    pub(crate) name: String,
    pub(crate) child: QualifiedTableName,
    pub(crate) parent: QualifiedTableName,
}

impl LiveSchema {
    /// Loads a catalog snapshot for the requested namespaces through the managed database boundary.
    pub(crate) async fn load(
        database: &Database,
        namespaces: &BTreeSet<String>,
    ) -> Result<Self, CliError> {
        let namespaces = namespaces.iter().cloned().collect::<Vec<_>>();
        let query_namespaces = namespaces.clone();
        let rows = database
            .run(move |connection| load_rows(connection, &query_namespaces))
            .await
            .map_err(|_| {
                catalog_error("PostgreSQL catalog queries could not be completed", None)
            })?;
        snapshot_from_rows(&namespaces.into_iter().collect(), rows)
    }

    /// Returns normalized live tables in qualified-name order.
    pub(crate) fn tables(&self) -> &BTreeMap<QualifiedTableName, TableSchema> {
        &self.tables
    }

    /// Returns unsupported object evidence in table/kind/name order.
    pub(crate) fn unsupported(&self) -> &[UnsupportedObject] {
        &self.unsupported
    }

    /// Returns foreign-key dependencies in child/parent/name order.
    pub(crate) fn foreign_keys(&self) -> &[ForeignKeyDependency] {
        &self.foreign_keys
    }
}

#[derive(Default)]
struct CatalogRows {
    columns: Vec<ColumnRow>,
    indexes: Vec<IndexRow>,
    checks: Vec<NamedObjectRow>,
    triggers: Vec<NamedObjectRow>,
    foreign_keys: Vec<ForeignKeyRow>,
}

#[derive(QueryableByName)]
struct ColumnRow {
    #[diesel(sql_type = Text)]
    schema: String,
    #[diesel(sql_type = Text)]
    table: String,
    #[diesel(sql_type = Integer)]
    ordinal: i32,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    catalog_type: String,
    #[diesel(sql_type = Bool)]
    nullable: bool,
    #[diesel(sql_type = Nullable<Integer>)]
    primary_key_position: Option<i32>,
    #[diesel(sql_type = Bool)]
    has_default: bool,
}

#[derive(QueryableByName)]
struct IndexRow {
    #[diesel(sql_type = Text)]
    schema: String,
    #[diesel(sql_type = Text)]
    table: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Array<Text>)]
    columns: Vec<String>,
    #[diesel(sql_type = Bool)]
    is_primary: bool,
}

#[derive(QueryableByName)]
struct NamedObjectRow {
    #[diesel(sql_type = Text)]
    schema: String,
    #[diesel(sql_type = Text)]
    table: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Array<Text>)]
    columns: Vec<String>,
}

#[derive(QueryableByName)]
struct ForeignKeyRow {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    child_schema: String,
    #[diesel(sql_type = Text)]
    child_table: String,
    #[diesel(sql_type = Text)]
    parent_schema: String,
    #[diesel(sql_type = Text)]
    parent_table: String,
    #[diesel(sql_type = Array<Text>)]
    columns: Vec<String>,
}

fn load_rows(connection: &mut PgConnection, namespaces: &[String]) -> QueryResult<CatalogRows> {
    let columns = sql_query(
        r#"
            SELECT n.nspname AS schema,
                   c.relname AS table,
                   a.attnum::integer AS ordinal,
                   a.attname AS name,
                   pg_catalog.format_type(a.atttypid, a.atttypmod) AS catalog_type,
                   NOT a.attnotnull AS nullable,
                   (
                       SELECT key.ordinality::integer
                       FROM pg_catalog.pg_index AS primary_index
                       CROSS JOIN LATERAL unnest(primary_index.indkey)
                           WITH ORDINALITY AS key(attnum, ordinality)
                       WHERE primary_index.indrelid = c.oid
                         AND primary_index.indisprimary
                         AND key.attnum = a.attnum
                       LIMIT 1
                   ) AS primary_key_position,
                   pg_catalog.pg_get_expr(default_value.adbin, default_value.adrelid)
                       IS NOT NULL AS has_default
            FROM pg_catalog.pg_attribute AS a
            JOIN pg_catalog.pg_class AS c ON c.oid = a.attrelid
            JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
            LEFT JOIN pg_catalog.pg_attrdef AS default_value
                ON default_value.adrelid = a.attrelid
               AND default_value.adnum = a.attnum
            WHERE n.nspname = ANY($1)
              AND c.relkind IN ('r', 'p')
              AND a.attnum > 0
              AND NOT a.attisdropped
              AND c.relname <> '__diesel_schema_migrations'
            ORDER BY n.nspname, c.relname, a.attnum
        "#,
    )
    .bind::<Array<Text>, _>(namespaces)
    .load(connection)?;

    let indexes = sql_query(
        r#"
            SELECT n.nspname AS schema,
                   table_class.relname AS table,
                   index_class.relname AS name,
                   COALESCE(
                       array_agg(attribute.attname ORDER BY key.ordinality)
                           FILTER (WHERE attribute.attname IS NOT NULL),
                       ARRAY[]::text[]
                   ) AS columns,
                   index_data.indisprimary AS is_primary
            FROM pg_catalog.pg_index AS index_data
            JOIN pg_catalog.pg_class AS table_class
                ON table_class.oid = index_data.indrelid
            JOIN pg_catalog.pg_namespace AS n
                ON n.oid = table_class.relnamespace
            JOIN pg_catalog.pg_class AS index_class
                ON index_class.oid = index_data.indexrelid
            CROSS JOIN LATERAL unnest(index_data.indkey)
                WITH ORDINALITY AS key(attnum, ordinality)
            LEFT JOIN pg_catalog.pg_attribute AS attribute
                ON attribute.attrelid = table_class.oid
               AND attribute.attnum = key.attnum
            WHERE n.nspname = ANY($1)
              AND table_class.relkind IN ('r', 'p')
              AND table_class.relname <> '__diesel_schema_migrations'
              AND NOT index_data.indisprimary
            GROUP BY n.nspname, table_class.relname, index_class.relname,
                     index_data.indisprimary
        "#,
    )
    .bind::<Array<Text>, _>(namespaces)
    .load(connection)?;

    let checks = sql_query(
        r#"
            SELECT n.nspname AS schema,
                   table_class.relname AS table,
                   constraint_data.conname AS name,
                   COALESCE(
                       array_agg(attribute.attname ORDER BY key.ordinality)
                           FILTER (WHERE attribute.attname IS NOT NULL),
                       ARRAY[]::text[]
                   ) AS columns
            FROM pg_catalog.pg_constraint AS constraint_data
            JOIN pg_catalog.pg_class AS table_class
                ON table_class.oid = constraint_data.conrelid
            JOIN pg_catalog.pg_namespace AS n
                ON n.oid = table_class.relnamespace
            LEFT JOIN LATERAL unnest(constraint_data.conkey)
                WITH ORDINALITY AS key(attnum, ordinality) ON TRUE
            LEFT JOIN pg_catalog.pg_attribute AS attribute
                ON attribute.attrelid = table_class.oid
               AND attribute.attnum = key.attnum
            WHERE n.nspname = ANY($1)
              AND table_class.relkind IN ('r', 'p')
              AND table_class.relname <> '__diesel_schema_migrations'
              AND constraint_data.contype = 'c'
            GROUP BY n.nspname, table_class.relname, constraint_data.conname
        "#,
    )
    .bind::<Array<Text>, _>(namespaces)
    .load(connection)?;

    let triggers = sql_query(
        r#"
            SELECT n.nspname AS schema,
                   table_class.relname AS table,
                   trigger_data.tgname AS name,
                   ARRAY[]::text[] AS columns
            FROM pg_catalog.pg_trigger AS trigger_data
            JOIN pg_catalog.pg_class AS table_class
                ON table_class.oid = trigger_data.tgrelid
            JOIN pg_catalog.pg_namespace AS n
                ON n.oid = table_class.relnamespace
            WHERE n.nspname = ANY($1)
              AND table_class.relkind IN ('r', 'p')
              AND table_class.relname <> '__diesel_schema_migrations'
              AND NOT trigger_data.tgisinternal
        "#,
    )
    .bind::<Array<Text>, _>(namespaces)
    .load(connection)?;

    let foreign_keys = sql_query(
        r#"
            SELECT constraint_data.conname AS name,
                   child_namespace.nspname AS child_schema,
                   child_class.relname AS child_table,
                   parent_namespace.nspname AS parent_schema,
                   parent_class.relname AS parent_table,
                   COALESCE(
                       array_agg(child_attribute.attname ORDER BY key.ordinality)
                           FILTER (WHERE child_attribute.attname IS NOT NULL),
                       ARRAY[]::text[]
                   ) AS columns
            FROM pg_catalog.pg_constraint AS constraint_data
            JOIN pg_catalog.pg_class AS child_class
                ON child_class.oid = constraint_data.conrelid
            JOIN pg_catalog.pg_namespace AS child_namespace
                ON child_namespace.oid = child_class.relnamespace
            JOIN pg_catalog.pg_class AS parent_class
                ON parent_class.oid = constraint_data.confrelid
            JOIN pg_catalog.pg_namespace AS parent_namespace
                ON parent_namespace.oid = parent_class.relnamespace
            LEFT JOIN LATERAL unnest(constraint_data.conkey)
                WITH ORDINALITY AS key(attnum, ordinality) ON TRUE
            LEFT JOIN pg_catalog.pg_attribute AS child_attribute
                ON child_attribute.attrelid = child_class.oid
               AND child_attribute.attnum = key.attnum
            WHERE child_namespace.nspname = ANY($1)
              AND child_class.relkind IN ('r', 'p')
              AND child_class.relname <> '__diesel_schema_migrations'
              AND constraint_data.contype = 'f'
            GROUP BY constraint_data.conname,
                     child_namespace.nspname, child_class.relname,
                     parent_namespace.nspname, parent_class.relname
        "#,
    )
    .bind::<Array<Text>, _>(namespaces)
    .load(connection)?;

    Ok(CatalogRows {
        columns,
        indexes,
        checks,
        triggers,
        foreign_keys,
    })
}

fn snapshot_from_rows(
    namespaces: &BTreeSet<String>,
    rows: CatalogRows,
) -> Result<LiveSchema, CliError> {
    let mut tables = BTreeMap::<QualifiedTableName, TableSchema>::new();
    let mut primary_keys = BTreeMap::<QualifiedTableName, Vec<(i32, String)>>::new();
    let mut unsupported = Vec::new();

    for row in rows
        .columns
        .into_iter()
        .filter(|row| namespaces.contains(&row.schema))
    {
        let table_name = QualifiedTableName::new(row.schema, row.table);
        let sql_type = parse_catalog_type(&row.catalog_type).ok_or_else(|| {
            catalog_error(
                "a column uses an unsupported PostgreSQL SQL type",
                Some(format!(
                    "{}.{}.{}",
                    table_name.schema, table_name.table, row.name
                )),
            )
        })?;
        let ordinal = usize::try_from(row.ordinal - 1).map_err(|_| {
            catalog_error(
                "PostgreSQL returned an invalid column ordinal",
                Some(format!(
                    "{}.{}.{}",
                    table_name.schema, table_name.table, row.name
                )),
            )
        })?;
        if let Some(position) = row.primary_key_position {
            primary_keys
                .entry(table_name.clone())
                .or_default()
                .push((position, row.name.clone()));
        }
        if row.has_default {
            unsupported.push(UnsupportedObject {
                kind: UnsupportedKind::Default,
                table: table_name.clone(),
                name: row.name.clone(),
                columns: vec![row.name.clone()],
            });
        }
        tables
            .entry(table_name.clone())
            .or_insert_with(|| TableSchema {
                name: table_name,
                columns: Vec::new(),
                primary_key: Vec::new(),
            })
            .columns
            .push(ColumnSchema {
                name: row.name,
                sql_type,
                nullable: row.nullable,
                ordinal,
            });
    }

    for (name, table) in &mut tables {
        table.columns.sort_by_key(|column| column.ordinal);
        if let Some(keys) = primary_keys.get_mut(name) {
            keys.sort_by_key(|(position, _)| *position);
            table.primary_key = keys.iter().map(|(_, column)| column.clone()).collect();
        }
    }

    unsupported.extend(rows.indexes.into_iter().filter_map(|row| {
        (!row.is_primary && namespaces.contains(&row.schema)).then(|| UnsupportedObject {
            kind: UnsupportedKind::Index,
            table: QualifiedTableName::new(row.schema, row.table),
            name: row.name,
            columns: row.columns,
        })
    }));
    unsupported.extend(named_objects(
        rows.checks,
        namespaces,
        UnsupportedKind::Check,
    ));
    unsupported.extend(named_objects(
        rows.triggers,
        namespaces,
        UnsupportedKind::Trigger,
    ));

    let mut foreign_keys = Vec::new();
    for row in rows
        .foreign_keys
        .into_iter()
        .filter(|row| namespaces.contains(&row.child_schema))
    {
        let child = QualifiedTableName::new(row.child_schema, row.child_table);
        let parent = QualifiedTableName::new(row.parent_schema, row.parent_table);
        unsupported.push(UnsupportedObject {
            kind: UnsupportedKind::ForeignKey,
            table: child.clone(),
            name: row.name.clone(),
            columns: row.columns,
        });
        foreign_keys.push(ForeignKeyDependency {
            name: row.name,
            child,
            parent,
        });
    }

    unsupported.sort_by(|left, right| {
        (&left.table, &left.kind, &left.name).cmp(&(&right.table, &right.kind, &right.name))
    });
    foreign_keys.sort_by(|left, right| {
        (&left.child, &left.parent, &left.name).cmp(&(&right.child, &right.parent, &right.name))
    });

    Ok(LiveSchema {
        tables,
        unsupported,
        foreign_keys,
    })
}

fn named_objects(
    rows: Vec<NamedObjectRow>,
    namespaces: &BTreeSet<String>,
    kind: UnsupportedKind,
) -> impl Iterator<Item = UnsupportedObject> + '_ {
    rows.into_iter()
        .filter(|row| namespaces.contains(&row.schema))
        .map(move |row| UnsupportedObject {
            kind: kind.clone(),
            table: QualifiedTableName::new(row.schema, row.table),
            name: row.name,
            columns: row.columns,
        })
}

fn parse_catalog_type(catalog_type: &str) -> Option<PgType> {
    if let Some(element) = catalog_type.strip_suffix("[]") {
        return parse_catalog_type(element).map(|kind| PgType::Array(Box::new(kind)));
    }
    if let Some(length) = type_length(catalog_type, "character varying") {
        return Some(PgType::VarChar(length));
    }
    if let Some(length) = type_length(catalog_type, "character") {
        return Some(PgType::Char(length));
    }
    if catalog_type == "numeric" || catalog_type.starts_with("numeric(") {
        return Some(PgType::Numeric);
    }
    match catalog_type {
        "boolean" | "bool" => Some(PgType::Bool),
        "smallint" | "int2" => Some(PgType::SmallInt),
        "integer" | "int4" => Some(PgType::Integer),
        "bigint" | "int8" => Some(PgType::BigInt),
        "real" | "float4" => Some(PgType::Real),
        "double precision" | "float8" => Some(PgType::DoublePrecision),
        "text" => Some(PgType::Text),
        "bytea" => Some(PgType::Bytea),
        "date" => Some(PgType::Date),
        "time without time zone" => Some(PgType::Time),
        "timestamp without time zone" => Some(PgType::Timestamp),
        "timestamp with time zone" => Some(PgType::TimestampWithTimeZone),
        "json" => Some(PgType::Json),
        "jsonb" => Some(PgType::Jsonb),
        "uuid" => Some(PgType::Uuid),
        "inet" => Some(PgType::Inet),
        "cidr" => Some(PgType::Cidr),
        "macaddr" => Some(PgType::MacAddr),
        _ => None,
    }
}

fn type_length(catalog_type: &str, base: &str) -> Option<Option<u32>> {
    if catalog_type == base {
        return Some(None);
    }
    catalog_type
        .strip_prefix(base)
        .and_then(|suffix| suffix.strip_prefix('('))
        .and_then(|suffix| suffix.strip_suffix(')'))
        .and_then(|length| length.parse().ok())
        .map(Some)
}

fn catalog_error(message: impl Into<String>, subject: Option<String>) -> CliError {
    let error = CliError::new(
        MADS211,
        "PostgreSQL catalog could not be inspected",
        message,
    );
    match subject {
        Some(subject) => error.with_subject(subject),
        None => error,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    use mads::{
        Database, DatabaseConfig,
        diesel::{Connection, PgConnection, RunQueryDsl, connection::SimpleConnection, sql_query},
    };

    use super::{
        CatalogRows, ColumnRow, ForeignKeyRow, IndexRow, LiveSchema, NamedObjectRow,
        UnsupportedKind, snapshot_from_rows,
    };
    use crate::{
        database::schema::{PgType, QualifiedTableName},
        diagnostic::MADS211,
    };

    #[test]
    fn normalizes_supported_catalog_types_and_reports_safe_unknown_context() {
        let supported = [
            ("boolean", PgType::Bool),
            ("smallint", PgType::SmallInt),
            ("integer", PgType::Integer),
            ("bigint", PgType::BigInt),
            ("real", PgType::Real),
            ("double precision", PgType::DoublePrecision),
            ("numeric", PgType::Numeric),
            ("text", PgType::Text),
            ("character varying", PgType::VarChar(None)),
            ("character varying(255)", PgType::VarChar(Some(255))),
            ("character", PgType::Char(None)),
            ("character(2)", PgType::Char(Some(2))),
            ("bytea", PgType::Bytea),
            ("date", PgType::Date),
            ("time without time zone", PgType::Time),
            ("timestamp without time zone", PgType::Timestamp),
            ("timestamp with time zone", PgType::TimestampWithTimeZone),
            ("json", PgType::Json),
            ("jsonb", PgType::Jsonb),
            ("uuid", PgType::Uuid),
            ("inet", PgType::Inet),
            ("cidr", PgType::Cidr),
            ("macaddr", PgType::MacAddr),
            ("text[]", PgType::Array(Box::new(PgType::Text))),
        ];
        for (catalog_type, expected) in supported {
            let rows = rows_with_column("audit", "events", "payload", catalog_type);
            let live = snapshot_from_rows(&BTreeSet::from(["audit".into()]), rows)
                .expect("supported catalog type should normalize");
            assert_eq!(
                live.tables()[&QualifiedTableName::new("audit", "events")].columns[0].sql_type,
                expected
            );
        }

        let error = snapshot_from_rows(
            &BTreeSet::from(["audit".into()]),
            rows_with_column("audit", "events", "payload", "secret_extension_type"),
        )
        .expect_err("unknown catalog type should fail");
        let rendered = error.to_string();
        assert_eq!(error.code(), MADS211);
        assert!(rendered.contains("audit.events.payload"));
        assert!(!rendered.contains("secret_extension_type"));
        assert!(!rendered.contains("postgres://"));
        assert!(!format!("{error:?}").contains("password"));
    }

    #[test]
    fn filters_namespaces_and_sorts_catalog_evidence_deterministically() {
        let mut rows = CatalogRows {
            columns: vec![
                column("ignored", "outside", "id", 1, "bigint", Some(1), false),
                column("zeta", "users", "id", 1, "bigint", Some(1), false),
                column("alpha", "accounts", "id", 1, "bigint", Some(1), false),
            ],
            indexes: vec![
                IndexRow {
                    schema: "zeta".into(),
                    table: "users".into(),
                    name: "users_pkey".into(),
                    columns: vec!["id".into()],
                    is_primary: true,
                },
                IndexRow {
                    schema: "zeta".into(),
                    table: "users".into(),
                    name: "z_users_email_idx".into(),
                    columns: vec!["email".into()],
                    is_primary: false,
                },
            ],
            checks: vec![named("alpha", "accounts", "z_check", vec!["id"])],
            triggers: vec![named("alpha", "accounts", "a_trigger", vec![])],
            foreign_keys: vec![
                ForeignKeyRow {
                    name: "z_fk".into(),
                    child_schema: "zeta".into(),
                    child_table: "users".into(),
                    parent_schema: "alpha".into(),
                    parent_table: "accounts".into(),
                    columns: vec!["account_id".into()],
                },
                ForeignKeyRow {
                    name: "a_fk".into(),
                    child_schema: "alpha".into(),
                    child_table: "accounts".into(),
                    parent_schema: "shared".into(),
                    parent_table: "tenants".into(),
                    columns: vec!["tenant_id".into()],
                },
                ForeignKeyRow {
                    name: "ignored_fk".into(),
                    child_schema: "ignored".into(),
                    child_table: "outside".into(),
                    parent_schema: "alpha".into(),
                    parent_table: "accounts".into(),
                    columns: vec!["id".into()],
                },
            ],
        };
        rows.columns[1].has_default = true;

        let live = snapshot_from_rows(&BTreeSet::from(["alpha".into(), "zeta".into()]), rows)
            .expect("catalog rows should normalize");

        assert_eq!(
            live.tables().keys().cloned().collect::<Vec<_>>(),
            [
                QualifiedTableName::new("alpha", "accounts"),
                QualifiedTableName::new("zeta", "users"),
            ]
        );
        assert!(
            !live
                .unsupported()
                .iter()
                .any(|item| item.name == "users_pkey")
        );
        assert_eq!(
            live.unsupported()
                .iter()
                .map(|item| (
                    item.table.schema.as_str(),
                    item.table.table.as_str(),
                    &item.kind,
                    item.name.as_str(),
                ))
                .collect::<Vec<_>>(),
            [
                ("alpha", "accounts", &UnsupportedKind::Check, "z_check"),
                ("alpha", "accounts", &UnsupportedKind::Trigger, "a_trigger"),
                ("alpha", "accounts", &UnsupportedKind::ForeignKey, "a_fk",),
                ("zeta", "users", &UnsupportedKind::Default, "id"),
                (
                    "zeta",
                    "users",
                    &UnsupportedKind::Index,
                    "z_users_email_idx",
                ),
                ("zeta", "users", &UnsupportedKind::ForeignKey, "z_fk"),
            ]
        );
        assert_eq!(
            live.foreign_keys()
                .iter()
                .map(|dependency| (
                    dependency.child.schema.as_str(),
                    dependency.child.table.as_str(),
                    dependency.parent.schema.as_str(),
                    dependency.parent.table.as_str(),
                    dependency.name.as_str(),
                ))
                .collect::<Vec<_>>(),
            [
                ("alpha", "accounts", "shared", "tenants", "a_fk"),
                ("zeta", "users", "alpha", "accounts", "z_fk"),
            ]
        );
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL through MADS_TEST_DATABASE_URL"]
    async fn catalog_snapshot() {
        let url = env::var("MADS_TEST_DATABASE_URL")
            .expect("MADS_TEST_DATABASE_URL is required for ignored PostgreSQL tests");
        let schema_name = format!(
            "mads_catalog_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should follow UNIX epoch")
                .as_nanos()
        );
        let cleanup = SchemaCleanup::new(url.clone(), schema_name.clone());
        let database = Database::from_config(
            &DatabaseConfig::new(url).expect("test database URL should be valid"),
        )
        .expect("test database pool should be created");
        let setup = format!(
            r#"
                CREATE SCHEMA "{schema_name}";
                CREATE TABLE "{schema_name}".users (
                    id bigint PRIMARY KEY,
                    email character varying(255) NOT NULL DEFAULT 'unknown',
                    nickname text NULL,
                    CONSTRAINT users_email_check CHECK (length(email) > 0)
                );
                CREATE INDEX users_nickname_idx ON "{schema_name}".users (nickname);
            "#
        );
        database
            .run(move |connection| connection.batch_execute(&setup))
            .await
            .expect("catalog fixture should be created");

        let live = LiveSchema::load(&database, &BTreeSet::from([schema_name.clone()]))
            .await
            .expect("catalog snapshot should load");
        let users = &live.tables()[&QualifiedTableName::new(&schema_name, "users")];
        assert_eq!(users.columns[0].sql_type, PgType::BigInt);
        assert_eq!(users.columns[1].sql_type, PgType::VarChar(Some(255)));
        assert!(!users.columns[1].nullable);
        assert!(users.columns[2].nullable);
        assert_eq!(users.primary_key, ["id"]);
        assert!(
            live.unsupported()
                .iter()
                .any(|item| { item.kind == UnsupportedKind::Default && item.columns == ["email"] })
        );
        assert!(live.unsupported().iter().any(|item| {
            item.kind == UnsupportedKind::Index && item.name == "users_nickname_idx"
        }));
        assert!(live.unsupported().iter().any(|item| {
            item.kind == UnsupportedKind::Check && item.name == "users_email_check"
        }));
        assert!(
            !live
                .unsupported()
                .iter()
                .any(|item| item.name.ends_with("_pkey"))
        );

        database.close();
        drop(cleanup);
    }

    fn rows_with_column(schema: &str, table: &str, name: &str, catalog_type: &str) -> CatalogRows {
        CatalogRows {
            columns: vec![column(schema, table, name, 1, catalog_type, None, true)],
            ..CatalogRows::default()
        }
    }

    fn column(
        schema: &str,
        table: &str,
        name: &str,
        ordinal: i32,
        catalog_type: &str,
        primary_key_position: Option<i32>,
        nullable: bool,
    ) -> ColumnRow {
        ColumnRow {
            schema: schema.into(),
            table: table.into(),
            ordinal,
            name: name.into(),
            catalog_type: catalog_type.into(),
            nullable,
            primary_key_position,
            has_default: false,
        }
    }

    fn named(schema: &str, table: &str, name: &str, columns: Vec<&str>) -> NamedObjectRow {
        NamedObjectRow {
            schema: schema.into(),
            table: table.into(),
            name: name.into(),
            columns: columns.into_iter().map(str::to_owned).collect(),
        }
    }

    struct SchemaCleanup {
        url: String,
        schema: String,
    }

    impl SchemaCleanup {
        fn new(url: String, schema: String) -> Self {
            Self { url, schema }
        }
    }

    impl Drop for SchemaCleanup {
        fn drop(&mut self) {
            let Ok(mut connection) = PgConnection::establish(&self.url) else {
                return;
            };
            let query = format!("DROP SCHEMA \"{}\" CASCADE", self.schema);
            let _ = sql_query(query).execute(&mut connection);
        }
    }
}
