# MADS.rs 0.4 PostgreSQL User Slice

This v0.4 example keeps persistence explicit: the composition root loads
configuration, registers one `DatabaseBootstrap`, and a repository uses native
Diesel through `Database::run`. Database errors below are deliberately mapped
by the controller; MADS does not normalize them into HTTP responses.

## Configuration

```toml
# mads.toml (tracked)
[database]
url = "${DATABASE_URL}"
pool_size = 10
migrate = false
```

Copy `.env.example` to the ignored `.env` for local use. In production or CI,
set `DATABASE_URL` as a process variable. To override the config key itself,
set `MADS_DATABASE__URL`; that final `MADS_` source wins over TOML and dotenv.

## `src/main.rs`

```rust,no_run
use mads::diesel::prelude::*;
use mads::{
    core::{ConfigBuilder, DotenvSource, EnvSource, TomlSource},
    diesel,
    prelude::*,
};

diesel::table! {
    users (id) {
        id -> Int8,
        name -> Varchar,
    }
}

#[derive(serde::Serialize, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = users)]
struct User {
    id: i64,
    name: String,
}

#[mads::repository]
struct UserRepository {
    database: Database,
}

impl UserRepository {
    async fn find(&self, id: i64) -> DatabaseResult<Option<User>> {
        self.database
            .run(move |connection| {
                users::table
                    .find(id)
                    .select(User::as_select())
                    .first(connection)
                    .optional()
            })
            .await
    }
}

#[mads::routes(prefix = "/users")]
trait UserRoutes {
    #[mads::get("/:id")]
    async fn get_user(&self, id: Path<i64>) -> HttpResult<Json<User>>;
}

#[mads::controller(routes = [UserRoutes])]
struct UserController {
    users: UserRepository,
}

impl UserRoutes for UserController {
    async fn get_user(&self, Path(id): Path<i64>) -> HttpResult<Json<User>> {
        self.users
            .find(id)
            .await
            .map_err(|_| HttpError::internal(std::io::Error::other("database query failed")))?
            .map(Json)
            .ok_or_else(|| HttpError::not_found("user was not found"))
    }
}

#[mads::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ConfigBuilder::new()
        .dotenv(DotenvSource::optional(".env"))
        .source(TomlSource::file("mads.toml"))
        .source(EnvSource::new("MADS_"))
        .build()?;
    let database = DatabaseConfig::from_config(&config)?;
    let mut builder = Mads::builder_with_config(config);
    builder.database(DatabaseBootstrap::new(database))?;
    let application = builder.build().await?;
    serve(application, "127.0.0.1:3000").await?;
    Ok(())
}
```

For startup migrations, declare an `EmbeddedMigrations` constant with
`mads::diesel_migrations::embed_migrations!("migrations")` and pass it to
`DatabaseBootstrap::with_migrations`; set `database.migrate = true` only when
that source is registered. File-based migration management is available through
`mads db migrate`, `mads db rollback`, and `mads db status`.

Use `mads::diesel` (or the direct `diesel` dependency shown above) for native
queries, schema macros, and Diesel traits. `Database::run` is the required
asynchronous boundary for synchronous PostgreSQL work.
