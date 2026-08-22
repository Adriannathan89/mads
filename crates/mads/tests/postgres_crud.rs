//! PostgreSQL CRUD acceptance coverage for the public MADS facade.

#![allow(missing_docs)]

use diesel::{self, prelude::*};
use mads::{
    axum::{
        body::{Body, Bytes, to_bytes},
        http::{Method, Request, StatusCode, header},
        response::Response,
    },
    diesel_migrations::{EmbeddedMigrations, embed_migrations},
    prelude::*,
};
use tower::ServiceExt;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("tests/fixtures/migrations");

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

diesel::table! {
    mads_v040_users (id) {
        id -> Int8,
        name -> Varchar,
    }
}

#[derive(Clone, Debug, diesel::Queryable, diesel::Selectable, serde::Serialize)]
#[diesel(table_name = mads_v040_users)]
struct User {
    id: i64,
    name: String,
}

#[derive(diesel::Insertable)]
#[diesel(table_name = mads_v040_users)]
struct NewUser {
    name: String,
}

#[derive(diesel::AsChangeset)]
#[diesel(table_name = mads_v040_users)]
struct UserChangeset {
    name: String,
}

#[mads::repository]
struct UserRepository {
    db: Database,
}

impl UserRepository {
    async fn create(&self, name: String) -> DatabaseResult<User> {
        self.db
            .run(move |connection| {
                diesel::insert_into(mads_v040_users::table)
                    .values(NewUser { name })
                    .returning(User::as_returning())
                    .get_result(connection)
            })
            .await
    }

    async fn list(&self) -> DatabaseResult<Vec<User>> {
        self.db
            .run(|connection| {
                mads_v040_users::table
                    .order(mads_v040_users::id.asc())
                    .select(User::as_select())
                    .load(connection)
            })
            .await
    }

    async fn get(&self, id: i64) -> DatabaseResult<Option<User>> {
        self.db
            .run(move |connection| {
                mads_v040_users::table
                    .find(id)
                    .select(User::as_select())
                    .first(connection)
                    .optional()
            })
            .await
    }

    async fn update(&self, id: i64, name: String) -> DatabaseResult<Option<User>> {
        self.db
            .run(move |connection| {
                diesel::update(mads_v040_users::table.find(id))
                    .set(UserChangeset { name })
                    .returning(User::as_returning())
                    .get_result(connection)
                    .optional()
            })
            .await
    }

    async fn delete(&self, id: i64) -> DatabaseResult<bool> {
        self.db
            .run(move |connection| {
                diesel::delete(mads_v040_users::table.find(id))
                    .execute(connection)
                    .map(|affected| affected > 0)
            })
            .await
    }
}

#[mads::service]
struct UserService {
    users: UserRepository,
}

impl UserService {
    async fn create(&self, name: String) -> DatabaseResult<User> {
        self.users.create(name).await
    }

    async fn list(&self) -> DatabaseResult<Vec<User>> {
        self.users.list().await
    }

    async fn get(&self, id: i64) -> DatabaseResult<Option<User>> {
        self.users.get(id).await
    }

    async fn update(&self, id: i64, name: String) -> DatabaseResult<Option<User>> {
        self.users.update(id, name).await
    }

    async fn delete(&self, id: i64) -> DatabaseResult<bool> {
        self.users.delete(id).await
    }
}

#[derive(serde::Deserialize)]
struct UserInput {
    name: String,
}

#[mads::routes(prefix = "/postgres-users")]
trait UserRoutes {
    #[mads::post("/")]
    async fn create(&self, input: Json<UserInput>) -> HttpResult<Created<Json<User>>>;

    #[mads::get("/")]
    async fn list(&self) -> HttpResult<Json<Vec<User>>>;

    #[mads::get("/:id")]
    async fn get(&self, id: Path<i64>) -> HttpResult<Json<User>>;

    #[mads::put("/:id")]
    async fn update(&self, id: Path<i64>, input: Json<UserInput>) -> HttpResult<Json<User>>;

    #[mads::delete("/:id")]
    async fn delete(&self, id: Path<i64>) -> HttpResult<NoContent>;
}

#[mads::controller(routes = [UserRoutes])]
struct UserController {
    users: UserService,
}

impl UserRoutes for UserController {
    async fn create(&self, Json(input): Json<UserInput>) -> HttpResult<Created<Json<User>>> {
        self.users
            .create(input.name)
            .await
            .map(|user| Created(Json(user)))
            .map_err(database_operation_error)
    }

    async fn list(&self) -> HttpResult<Json<Vec<User>>> {
        self.users
            .list()
            .await
            .map(Json)
            .map_err(database_operation_error)
    }

    async fn get(&self, Path(id): Path<i64>) -> HttpResult<Json<User>> {
        self.users
            .get(id)
            .await
            .map_err(database_operation_error)?
            .map(Json)
            .ok_or_else(|| HttpError::not_found("user was not found"))
    }

    async fn update(
        &self,
        Path(id): Path<i64>,
        Json(input): Json<UserInput>,
    ) -> HttpResult<Json<User>> {
        self.users
            .update(id, input.name)
            .await
            .map_err(database_operation_error)?
            .map(Json)
            .ok_or_else(|| HttpError::not_found("user was not found"))
    }

    async fn delete(&self, Path(id): Path<i64>) -> HttpResult<NoContent> {
        self.users
            .delete(id)
            .await
            .map_err(database_operation_error)?
            .then_some(NoContent)
            .ok_or_else(|| HttpError::not_found("user was not found"))
    }
}

fn database_operation_error(_: DatabaseError) -> HttpError {
    HttpError::internal(std::io::Error::other("database operation failed"))
}

#[test]
fn crud_application_has_a_repository_database_dependency() {
    let repository = mads::core::Catalog::provider_for::<UserRepository>()
        .expect("the repository declaration should be registered");

    assert_eq!(repository.dependencies().len(), 1);
    assert_eq!(repository.dependencies()[0].type_name(), "Database");
}

#[tokio::test]
#[ignore = "requires PostgreSQL through MADS_TEST_DATABASE_URL"]
async fn postgres_crud_uses_the_managed_graph_and_generated_http_routes() {
    let _guard = TEST_LOCK.lock().await;
    let config = DatabaseConfig::new(
        std::env::var("MADS_TEST_DATABASE_URL")
            .expect("MADS_TEST_DATABASE_URL is required for ignored PostgreSQL tests"),
    )
    .unwrap()
    .with_pool_size(2)
    .unwrap()
    .with_migrate_on_startup(true);
    let mut builder = Mads::builder();
    builder
        .database(DatabaseBootstrap::new(config).with_migrations(MIGRATIONS))
        .unwrap();
    let mut application = builder.build().await.unwrap();
    let router = build_router(&application).unwrap();

    assert_eq!(
        application
            .graph()
            .provider::<Database>()
            .expect("database should be registered in the graph")
            .origin(),
        ProviderOrigin::Provided
    );
    assert_dependency::<UserController, UserService>(&application);
    assert_dependency::<UserService, UserRepository>(&application);
    assert_dependency::<UserRepository, Database>(&application);

    application.start().await.unwrap();
    let database = application
        .context()
        .resolve::<Database>()
        .unwrap()
        .as_ref()
        .clone();
    database
        .run(|connection| diesel::delete(mads_v040_users::table).execute(connection))
        .await
        .unwrap();

    let create = router
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/postgres-users",
            r#"{"name":"Ada"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let create = json_body(create).await;
    let id = create["id"]
        .as_i64()
        .expect("the created response should contain an integer id");
    assert_eq!(create["name"], "Ada");

    let list = router
        .clone()
        .oneshot(empty_request(Method::GET, "/postgres-users"))
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    assert_eq!(
        json_body(list).await,
        serde_json::json!([{ "id": id, "name": "Ada" }])
    );

    let get = router
        .clone()
        .oneshot(empty_request(Method::GET, &format!("/postgres-users/{id}")))
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);
    assert_eq!(
        json_body(get).await,
        serde_json::json!({ "id": id, "name": "Ada" })
    );

    let update = router
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/postgres-users/{id}"),
            r#"{"name":"Grace"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);
    assert_eq!(
        json_body(update).await,
        serde_json::json!({ "id": id, "name": "Grace" })
    );

    let delete = router
        .clone()
        .oneshot(empty_request(
            Method::DELETE,
            &format!("/postgres-users/{id}"),
        ))
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::NO_CONTENT);
    assert!(response_body(delete).await.is_empty());

    let missing = router
        .oneshot(empty_request(Method::GET, &format!("/postgres-users/{id}")))
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        json_body(missing).await,
        serde_json::json!({
            "error": { "code": "not_found", "message": "user was not found" }
        })
    );

    application.shutdown().await.unwrap();
    assert!(database.is_closed());
}

fn assert_dependency<Provider, Dependency>(application: &Mads)
where
    Provider: Send + Sync + 'static,
    Dependency: Send + Sync + 'static,
{
    assert!(application.graph().dependencies().iter().any(|edge| {
        edge.provider_type_name() == std::any::type_name::<Provider>()
            && edge.dependency_type_name() == std::any::type_name::<Dependency>()
    }));
}

fn json_request(method: Method, uri: &str, body: &'static str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

fn empty_request(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

async fn response_body(response: Response) -> Bytes {
    to_bytes(response.into_body(), usize::MAX).await.unwrap()
}

async fn json_body(response: Response) -> serde_json::Value {
    serde_json::from_slice(&response_body(response).await).unwrap()
}
