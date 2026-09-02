use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    ops::Index,
    path::{Path, PathBuf},
};

use diesel_table_macro_syntax::TableDecl;
use syn::{Attribute, File, Item, ItemMacro, ItemMod, Type, TypePath, ext::IdentExt};

use crate::diagnostic::{CliError, MADS210};

/// A schema-qualified PostgreSQL table name.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct QualifiedTableName {
    pub(crate) schema: String,
    pub(crate) table: String,
}

impl QualifiedTableName {
    /// Constructs a qualified table name from owned-compatible identifiers.
    pub(crate) fn new(schema: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            schema: schema.into(),
            table: table.into(),
        }
    }

    fn display_name(&self) -> String {
        format!("{}.{}", self.schema, self.table)
    }
}

/// One ordered column in a desired table declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ColumnSchema {
    pub(crate) name: String,
    pub(crate) sql_type: PgType,
    pub(crate) nullable: bool,
    pub(crate) ordinal: usize,
}

/// A table declaration normalized from Diesel source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TableSchema {
    pub(crate) name: QualifiedTableName,
    pub(crate) columns: Vec<ColumnSchema>,
    pub(crate) primary_key: Vec<String>,
}

/// A PostgreSQL type supported by schema-diff generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PgType {
    /// PostgreSQL `boolean`.
    Bool,
    /// PostgreSQL `smallint`.
    SmallInt,
    /// PostgreSQL `integer`.
    Integer,
    /// PostgreSQL `bigint`.
    BigInt,
    /// PostgreSQL `real`.
    Real,
    /// PostgreSQL `double precision`.
    DoublePrecision,
    /// PostgreSQL `numeric`.
    Numeric,
    /// PostgreSQL `text`.
    Text,
    /// PostgreSQL `varchar`, optionally constrained by length.
    VarChar(Option<u32>),
    /// PostgreSQL `char`, optionally constrained by length.
    Char(Option<u32>),
    /// PostgreSQL `bytea`.
    Bytea,
    /// PostgreSQL `date`.
    Date,
    /// PostgreSQL `time`.
    Time,
    /// PostgreSQL `timestamp`.
    Timestamp,
    /// PostgreSQL `timestamp with time zone`.
    TimestampWithTimeZone,
    /// PostgreSQL `json`.
    Json,
    /// PostgreSQL `jsonb`.
    Jsonb,
    /// PostgreSQL `uuid`.
    Uuid,
    /// PostgreSQL `inet`.
    Inet,
    /// PostgreSQL `cidr`.
    Cidr,
    /// PostgreSQL `macaddr`.
    MacAddr,
    /// A PostgreSQL array of another supported type.
    Array(Box<PgType>),
}

/// The desired database schema normalized from one or more Diesel declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DesiredSchema {
    tables: BTreeMap<QualifiedTableName, TableSchema>,
    source_files: Vec<PathBuf>,
}

impl DesiredSchema {
    /// Loads regular Diesel schema sources rooted at a selected package.
    pub(crate) fn load(root: &Path) -> Result<Self, CliError> {
        let canonical_root = fs::canonicalize(root).map_err(|_| {
            schema_error(
                "the selected package root could not be read",
                Some(root.display().to_string()),
            )
        })?;
        let source_files = discover_sources(&canonical_root)?;
        if source_files.is_empty() {
            return Err(schema_error(
                "no regular `src/schema.rs` or `src/schema/**/*.rs` source was found",
                None,
            ));
        }

        let mut tables = BTreeMap::new();
        let mut table_sources = BTreeMap::new();
        for source in &source_files {
            let absolute = canonical_root.join(source);
            let contents = fs::read_to_string(&absolute).map_err(|_| {
                schema_error(
                    "a schema source could not be read as UTF-8",
                    Some(source.display().to_string()),
                )
            })?;
            let file = syn::parse_file(&contents).map_err(|_| {
                schema_error(
                    "a schema source contains malformed Rust",
                    Some(source.display().to_string()),
                )
            })?;
            visit_file(&file, source, &mut tables, &mut table_sources)?;
        }

        Ok(Self {
            tables,
            source_files,
        })
    }

    /// Returns the desired tables in qualified-name order.
    pub(crate) fn tables(&self) -> &BTreeMap<QualifiedTableName, TableSchema> {
        &self.tables
    }

    /// Returns source paths relative to the selected package root.
    pub(crate) fn source_files(&self) -> &[PathBuf] {
        &self.source_files
    }

    /// Returns the desired namespaces in lexical order.
    pub(crate) fn namespaces(&self) -> BTreeSet<String> {
        self.tables.keys().map(|name| name.schema.clone()).collect()
    }

    #[cfg(test)]
    fn table_names(&self) -> Vec<String> {
        self.tables
            .keys()
            .map(QualifiedTableName::display_name)
            .collect()
    }
}

impl Index<&str> for DesiredSchema {
    type Output = TableSchema;

    fn index(&self, name: &str) -> &Self::Output {
        let (schema, table) = name
            .split_once('.')
            .expect("desired schema lookup must use schema.table");
        self.tables
            .get(&QualifiedTableName::new(schema, table))
            .expect("desired schema lookup must name an existing table")
    }
}

fn discover_sources(root: &Path) -> Result<Vec<PathBuf>, CliError> {
    let mut sources = Vec::new();
    let schema_file = root.join("src/schema.rs");
    if regular_file(&schema_file)? {
        sources.push(relative_source(root, &schema_file)?);
    }

    let schema_directory = root.join("src/schema");
    if regular_directory(&schema_directory)? {
        discover_directory(root, &schema_directory, &mut sources)?;
    }

    sources.sort_by(|left, right| left.as_os_str().cmp(right.as_os_str()));
    Ok(sources)
}

fn discover_directory(
    root: &Path,
    directory: &Path,
    sources: &mut Vec<PathBuf>,
) -> Result<(), CliError> {
    let entries = fs::read_dir(directory).map_err(|_| {
        schema_error(
            "a schema directory could not be read",
            relative_display(root, directory),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|_| {
            schema_error(
                "a schema directory entry could not be read",
                relative_display(root, directory),
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|_| {
            schema_error(
                "a schema source type could not be read",
                relative_display(root, &path),
            )
        })?;
        if file_type.is_dir() {
            discover_directory(root, &path, sources)?;
        } else if file_type.is_file() && path.extension().is_some_and(|extension| extension == "rs")
        {
            sources.push(relative_source(root, &path)?);
        }
    }
    Ok(())
}

fn regular_file(path: &Path) -> Result<bool, CliError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(schema_error(
            "a schema source could not be inspected",
            Some(path.display().to_string()),
        )),
    }
}

fn regular_directory(path: &Path) -> Result<bool, CliError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_dir()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(schema_error(
            "a schema directory could not be inspected",
            Some(path.display().to_string()),
        )),
    }
}

fn relative_source(root: &Path, source: &Path) -> Result<PathBuf, CliError> {
    let canonical_source = fs::canonicalize(source).map_err(|_| {
        schema_error(
            "a schema source could not be canonicalized",
            relative_display(root, source),
        )
    })?;
    let relative = canonical_source.strip_prefix(root).map_err(|_| {
        schema_error(
            "a schema source resolves outside the selected package",
            relative_display(root, source),
        )
    })?;
    Ok(relative.to_path_buf())
}

fn relative_display(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|relative| relative.display().to_string())
}

fn visit_file(
    file: &File,
    source: &Path,
    tables: &mut BTreeMap<QualifiedTableName, TableSchema>,
    table_sources: &mut BTreeMap<QualifiedTableName, PathBuf>,
) -> Result<(), CliError> {
    reject_conditional_attributes(&file.attrs, source)?;
    visit_items(&file.items, source, tables, table_sources)
}

fn visit_items(
    items: &[Item],
    source: &Path,
    tables: &mut BTreeMap<QualifiedTableName, TableSchema>,
    table_sources: &mut BTreeMap<QualifiedTableName, PathBuf>,
) -> Result<(), CliError> {
    for item in items {
        reject_conditional_attributes(item_attrs(item), source)?;
        match item {
            Item::Macro(item_macro) if ends_with_table(&item_macro.mac.path) => {
                add_table(item_macro, source, tables, table_sources)?;
            }
            Item::Mod(ItemMod {
                content: Some((_, inline_items)),
                ..
            }) => visit_items(inline_items, source, tables, table_sources)?,
            _ => {}
        }
    }
    Ok(())
}

fn item_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => &[],
        _ => &[],
    }
}

fn reject_conditional_attributes(attributes: &[Attribute], source: &Path) -> Result<(), CliError> {
    if attributes.iter().any(|attribute| {
        attribute
            .path()
            .get_ident()
            .is_some_and(|name| name == "cfg" || name == "cfg_attr")
    }) {
        return Err(schema_error(
            "`cfg` and `cfg_attr` are unsupported in schema sources",
            Some(source.display().to_string()),
        ));
    }
    Ok(())
}

fn ends_with_table(path: &syn::Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == "table")
}

fn add_table(
    item: &ItemMacro,
    source: &Path,
    tables: &mut BTreeMap<QualifiedTableName, TableSchema>,
    table_sources: &mut BTreeMap<QualifiedTableName, PathBuf>,
) -> Result<(), CliError> {
    let declaration = syn::parse2::<TableDecl>(item.mac.tokens.clone()).map_err(|_| {
        schema_error(
            "a `table!` declaration has unsupported or malformed syntax",
            Some(source.display().to_string()),
        )
    })?;
    reject_inert_attributes(&declaration.meta, source)?;
    for use_statement in &declaration.use_statements {
        reject_conditional_attributes(&use_statement.attrs, source)?;
    }
    let name = QualifiedTableName::new(
        declaration
            .schema
            .as_ref()
            .map(ident_name)
            .unwrap_or_else(|| "public".to_owned()),
        declaration.sql_name.clone(),
    );
    let columns = declaration
        .column_defs
        .iter()
        .enumerate()
        .map(|(ordinal, column)| {
            reject_inert_attributes(&column.meta, source)?;
            let mut parsed = parse_sql_type(&column.tpe, source)?;
            if let Some(length) = &column.max_length {
                let length = length.base10_parse::<u32>().map_err(|_| {
                    schema_error(
                        "`max_length` must be a valid unsigned 32-bit integer",
                        Some(source.display().to_string()),
                    )
                })?;
                match &mut parsed.sql_type {
                    PgType::VarChar(max_length) | PgType::Char(max_length) => {
                        *max_length = Some(length);
                    }
                    _ => {
                        return Err(schema_error(
                            "`max_length` is supported only for `VarChar` and `Char` columns",
                            Some(source.display().to_string()),
                        ));
                    }
                }
            }
            Ok(ColumnSchema {
                name: column.sql_name.clone(),
                sql_type: parsed.sql_type,
                nullable: parsed.nullable,
                ordinal,
            })
        })
        .collect::<Result<Vec<_>, CliError>>()?;
    let primary_key = primary_key(&declaration, &columns, source)?;
    let table = TableSchema {
        name: name.clone(),
        columns,
        primary_key,
    };

    if let Some(previous_source) = table_sources.get(&name) {
        return Err(schema_error(
            "a qualified table is declared by more than one schema source",
            Some(format!(
                "{} and {}",
                previous_source.display(),
                source.display()
            )),
        ));
    }
    table_sources.insert(name.clone(), source.to_path_buf());
    tables.insert(name, table);
    Ok(())
}

fn reject_inert_attributes(attributes: &[Attribute], source: &Path) -> Result<(), CliError> {
    if attributes.iter().all(|attribute| {
        attribute.path().is_ident("doc")
            || attribute.path().is_ident("allow")
            || attribute.path().is_ident("warn")
            || attribute.path().is_ident("deny")
            || attribute.path().is_ident("forbid")
    }) {
        return Ok(());
    }
    Err(schema_error(
        "a `table!` declaration contains an unsupported attribute",
        Some(source.display().to_string()),
    ))
}

fn primary_key(
    declaration: &TableDecl,
    columns: &[ColumnSchema],
    source: &Path,
) -> Result<Vec<String>, CliError> {
    let key_names = declaration
        .primary_keys
        .as_ref()
        .map(|primary_keys| primary_keys.keys.iter().map(ident_name).collect())
        .unwrap_or_else(|| vec!["id".to_owned()]);
    key_names
        .into_iter()
        .map(|key| {
            declaration
                .column_defs
                .iter()
                .position(|column| ident_name(&column.column_name) == key)
                .and_then(|index| columns.get(index))
                .map(|column| column.name.clone())
                .ok_or_else(|| {
                    schema_error(
                        "a primary-key column is not declared by its `table!` declaration",
                        Some(source.display().to_string()),
                    )
                })
        })
        .collect()
}

fn ident_name(ident: &syn::Ident) -> String {
    ident.unraw().to_string()
}

struct ParsedSqlType {
    sql_type: PgType,
    nullable: bool,
}

fn parse_sql_type(type_path: &TypePath, source: &Path) -> Result<ParsedSqlType, CliError> {
    if type_path.qself.is_some()
        || type_path.path.leading_colon.is_some()
        || type_path.path.segments.len() != 1
    {
        return Err(unknown_type(source));
    }
    let mut current = type_path;
    let mut nullable = false;
    while type_name(current).as_deref() == Some("Nullable") {
        nullable = true;
        current = nested_type(current, source)?;
    }
    Ok(ParsedSqlType {
        sql_type: parse_non_nullable_type(current, source)?,
        nullable,
    })
}

fn parse_non_nullable_type(type_path: &TypePath, source: &Path) -> Result<PgType, CliError> {
    let name = type_name(type_path).ok_or_else(|| unknown_type(source))?;
    if name == "Array" {
        let nested = nested_type(type_path, source)?;
        let parsed = parse_sql_type(nested, source)?;
        if parsed.nullable {
            return Err(schema_error(
                "`Nullable` may wrap only the complete SQL type, not an array element",
                Some(source.display().to_string()),
            ));
        }
        return Ok(PgType::Array(Box::new(parsed.sql_type)));
    }

    let no_arguments = type_path
        .path
        .segments
        .last()
        .is_some_and(|segment| matches!(segment.arguments, syn::PathArguments::None));
    if !no_arguments {
        return Err(unknown_type(source));
    }
    match name.as_str() {
        "Bool" => Ok(PgType::Bool),
        "Int2" | "SmallInt" => Ok(PgType::SmallInt),
        "Int4" | "Integer" => Ok(PgType::Integer),
        "Int8" | "BigInt" => Ok(PgType::BigInt),
        "Float4" | "Float" | "Real" => Ok(PgType::Real),
        "Double" | "DoublePrecision" | "Float8" => Ok(PgType::DoublePrecision),
        "Numeric" => Ok(PgType::Numeric),
        "Text" => Ok(PgType::Text),
        "VarChar" | "Varchar" => Ok(PgType::VarChar(None)),
        "Char" => Ok(PgType::Char(None)),
        "Bytea" => Ok(PgType::Bytea),
        "Date" => Ok(PgType::Date),
        "Time" => Ok(PgType::Time),
        "Timestamp" => Ok(PgType::Timestamp),
        "TimestampWithTimeZone" | "Timestamptz" => Ok(PgType::TimestampWithTimeZone),
        "Json" => Ok(PgType::Json),
        "Jsonb" => Ok(PgType::Jsonb),
        "Uuid" => Ok(PgType::Uuid),
        "Inet" => Ok(PgType::Inet),
        "Cidr" => Ok(PgType::Cidr),
        "MacAddr" => Ok(PgType::MacAddr),
        _ => Err(unknown_type(source)),
    }
}

fn type_name(type_path: &TypePath) -> Option<String> {
    type_path
        .path
        .segments
        .last()
        .map(|segment| ident_name(&segment.ident))
}

fn nested_type<'a>(type_path: &'a TypePath, source: &Path) -> Result<&'a TypePath, CliError> {
    let Some(segment) = type_path.path.segments.last() else {
        return Err(unknown_type(source));
    };
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(unknown_type(source));
    };
    let mut arguments = arguments.args.iter();
    let Some(argument) = arguments.next() else {
        return Err(unknown_type(source));
    };
    if arguments.next().is_some() {
        return Err(unknown_type(source));
    }
    let syn::GenericArgument::Type(Type::Path(nested)) = argument else {
        return Err(unknown_type(source));
    };
    Ok(nested)
}

fn unknown_type(source: &Path) -> CliError {
    schema_error(
        "a column uses an unsupported PostgreSQL SQL type",
        Some(source.display().to_string()),
    )
}

fn schema_error(message: impl Into<String>, subject: Option<String>) -> CliError {
    let error = CliError::new(MADS210, "Diesel schema source could not be loaded", message);
    match subject {
        Some(subject) => error.with_subject(subject),
        None => error,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use tempfile::tempdir;

    use super::{ColumnSchema, DesiredSchema, PgType};
    use crate::diagnostic::{CliError, MADS210};

    #[test]
    fn loads_schema_rs_and_recursive_schema_directory_in_lexical_order() {
        let root = tempdir().expect("temporary project should be created");
        write_schema(
            root.path(),
            "src/schema.rs",
            "diesel::table! { accounts { id -> Int8, } }",
        );
        write_schema(
            root.path(),
            "src/schema/comment.rs",
            "diesel::table! { comments { id -> Int8, } }",
        );
        write_schema(
            root.path(),
            "src/schema/nested/tag.rs",
            "diesel::table! { tags { id -> Int8, } }",
        );

        let schema = DesiredSchema::load(root.path()).expect("schema sources should load");

        assert_eq!(
            schema.table_names(),
            ["public.accounts", "public.comments", "public.tags"],
        );
        assert_eq!(
            schema.source_files(),
            [
                Path::new("src/schema.rs"),
                Path::new("src/schema/comment.rs"),
                Path::new("src/schema/nested/tag.rs"),
            ],
        );
    }

    #[test]
    fn loads_split_fixture_with_stable_tables_and_source_files() {
        let schema = DesiredSchema::load(&fixture("split")).expect("split fixture should load");

        assert_eq!(schema.table_names(), ["public.comments", "public.users"]);
        assert_eq!(
            schema.source_files(),
            [
                Path::new("src/schema/comment.rs"),
                Path::new("src/schema/user.rs"),
            ],
        );
    }

    #[test]
    fn loads_the_single_schema_fixture() {
        let schema = DesiredSchema::load(&fixture("single")).expect("single fixture should load");

        assert_eq!(schema.table_names(), ["public.users"]);
        assert_eq!(schema.source_files(), [Path::new("src/schema.rs")]);
    }

    #[test]
    fn defaults_primary_key_to_id_and_preserves_composites() {
        let schema = parse_schema(
            r#"
                diesel::table! { users { id -> Int8, name -> Varchar, } }
                diesel::table! { memberships (user_id, team_id) {
                    user_id -> Int8, team_id -> Int8,
                } }
            "#,
        )
        .expect("schema should parse");

        assert_eq!(schema["public.users"].primary_key, ["id"]);
        assert_eq!(
            schema["public.memberships"].primary_key,
            ["user_id", "team_id"]
        );
    }

    #[test]
    fn preserves_sql_names_lengths_and_explicit_schema() {
        let schema = parse_schema(
            r#"
                diesel::table! {
                    #[sql_name = "event_log"]
                    audit.events (event_id) {
                        #[sql_name = "event-id"]
                        event_id -> Int8,
                        #[max_length = 42]
                        summary -> VarChar,
                        #[max_length = 1]
                        kind -> Char,
                    }
                }
            "#,
        )
        .expect("schema should parse");

        let events = &schema["audit.event_log"];
        assert_eq!(events.primary_key, ["event-id"]);
        assert_eq!(
            events.columns,
            [
                ColumnSchema {
                    name: "event-id".into(),
                    sql_type: PgType::BigInt,
                    nullable: false,
                    ordinal: 0,
                },
                ColumnSchema {
                    name: "summary".into(),
                    sql_type: PgType::VarChar(Some(42)),
                    nullable: false,
                    ordinal: 1,
                },
                ColumnSchema {
                    name: "kind".into(),
                    sql_type: PgType::Char(Some(1)),
                    nullable: false,
                    ordinal: 2,
                },
            ]
        );
    }

    #[test]
    fn unwraps_nullable_array_columns() {
        let schema =
            parse_schema("diesel::table! { posts { id -> Int8, tags -> Nullable<Array<Text>>, } }")
                .expect("schema should parse");

        assert_eq!(
            schema["public.posts"].columns[1],
            ColumnSchema {
                name: "tags".into(),
                sql_type: PgType::Array(Box::new(PgType::Text)),
                nullable: true,
                ordinal: 1,
            }
        );
    }

    #[test]
    fn visits_inline_modules_and_normalizes_supported_type_aliases() {
        let schema = parse_schema(
            r#"
                mod nested {
                    diesel::table! {
                        every_type {
                            id -> Bool,
                            int2 -> Int2,
                            small_int -> SmallInt,
                            int4 -> Int4,
                            integer -> Integer,
                            int8 -> Int8,
                            big_int -> BigInt,
                            float4 -> Float4,
                            float -> Float,
                            real -> Real,
                            float8 -> Float8,
                            double -> Double,
                            double_precision -> DoublePrecision,
                            numeric -> Numeric,
                            text -> Text,
                            varchar -> Varchar,
                            var_char -> VarChar,
                            char_value -> Char,
                            bytea -> Bytea,
                            date -> Date,
                            time -> Time,
                            timestamp -> Timestamp,
                            timestamptz -> Timestamptz,
                            timestamp_with_time_zone -> TimestampWithTimeZone,
                            json -> Json,
                            jsonb -> Jsonb,
                            uuid -> Uuid,
                            inet -> Inet,
                            cidr -> Cidr,
                            mac_addr -> MacAddr,
                            text_array -> Array<Text>,
                        }
                    }
                }
            "#,
        )
        .expect("supported Diesel SQL types should parse");

        assert_eq!(
            schema["public.every_type"]
                .columns
                .iter()
                .map(|column| column.sql_type.clone())
                .collect::<Vec<_>>(),
            vec![
                PgType::Bool,
                PgType::SmallInt,
                PgType::SmallInt,
                PgType::Integer,
                PgType::Integer,
                PgType::BigInt,
                PgType::BigInt,
                PgType::Real,
                PgType::Real,
                PgType::Real,
                PgType::DoublePrecision,
                PgType::DoublePrecision,
                PgType::DoublePrecision,
                PgType::Numeric,
                PgType::Text,
                PgType::VarChar(None),
                PgType::VarChar(None),
                PgType::Char(None),
                PgType::Bytea,
                PgType::Date,
                PgType::Time,
                PgType::Timestamp,
                PgType::TimestampWithTimeZone,
                PgType::TimestampWithTimeZone,
                PgType::Json,
                PgType::Jsonb,
                PgType::Uuid,
                PgType::Inet,
                PgType::Cidr,
                PgType::MacAddr,
                PgType::Array(Box::new(PgType::Text)),
            ]
        );
    }

    #[test]
    fn duplicate_tables_name_both_relative_sources() {
        let root = tempdir().expect("temporary project should be created");
        write_schema(
            root.path(),
            "src/schema/a.rs",
            "diesel::table! { users { id -> Int8, } }",
        );
        write_schema(
            root.path(),
            "src/schema/b.rs",
            "diesel::table! { users { id -> Int8, } }",
        );

        let error = DesiredSchema::load(root.path()).expect_err("duplicate table should fail");

        assert_eq!(error.code(), MADS210);
        assert!(error.to_string().contains("src/schema/a.rs"));
        assert!(error.to_string().contains("src/schema/b.rs"));
    }

    #[test]
    fn rejects_missing_malformed_conditional_and_unknown_sources() {
        let missing = tempdir().expect("temporary project should be created");
        assert_eq!(
            DesiredSchema::load(missing.path())
                .expect_err("missing source should fail")
                .code(),
            MADS210
        );

        for (name, source) in [
            ("malformed Rust", "diesel::table! { users { id -> Int8, }"),
            ("malformed table", "diesel::table! { users { id Int8, } }"),
            (
                "conditional table",
                "#[cfg(feature = \"postgres\")] diesel::table! { users { id -> Int8, } }",
            ),
            (
                "conditional attribute table",
                "#[cfg_attr(feature = \"postgres\", allow(dead_code))] diesel::table! { users { id -> Int8, } }",
            ),
            (
                "unknown SQL type",
                "diesel::table! { users { id -> CustomSqlType, } }",
            ),
            (
                "qualified custom SQL type",
                "diesel::table! { users { id -> custom::Text, } }",
            ),
            (
                "nullable array element",
                "diesel::table! { users { id -> Array<Nullable<Text>>, } }",
            ),
            (
                "max length on text",
                "diesel::table! { users { #[max_length = 1] id -> Text, } }",
            ),
        ] {
            let error = parse_schema(source).expect_err(name);
            assert_eq!(error.code(), MADS210, "{name}");
        }
    }

    fn parse_schema(source: &str) -> Result<DesiredSchema, CliError> {
        let root = tempdir().expect("temporary project should be created");
        write_schema(root.path(), "src/schema.rs", source);
        DesiredSchema::load(root.path())
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/schema")
            .join(name)
    }

    fn write_schema(root: &Path, relative: &str, source: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("schema file has parent"))
            .expect("schema parent should be created");
        fs::write(path, source).expect("schema source should be written");
    }
}
