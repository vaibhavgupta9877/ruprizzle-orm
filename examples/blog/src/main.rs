mod db;

#[tokio::main]
async fn main() -> Result<(), ruprizzle::Error> {
    dotenvy::dotenv().ok();
    let db_url =
        std::env::var("DATABASE_URL").map_err(|e| ruprizzle::Error::Message(e.to_string()))?;
    let db = db::Db::connect(&db_url).await?;

    let author = db
        .user()
        .create(db::UserInsert {
            id: None,
            email: "alice@example.com".into(),
            name: Some("Alice".into()),
            role: Some(db::enums::Role::Admin),
            created_at: None,
            updated_at: None,
        })
        .exec()
        .await?;

    let _post = db
        .post()
        .create(db::PostInsert {
            id: None,
            title: "Hello, ruprizzle".into(),
            body: Some("This is the first post.".into()),
            published: Some(true),
            author_id: author.id,
            created_at: None,
        })
        .exec()
        .await?;

    let posts = db
        .post()
        .find_many()
        .filter(db::post::PUBLISHED.eq(true))
        .order_by(db::post::CREATED_AT.desc())
        .include(db::post::author())
        .limit(10)
        .exec()
        .await?;

    for post in &posts {
        let author_name = post
            .author
            .get()
            .as_ref()
            .and_then(|a| a.name.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        println!("{} by {}", post.title, author_name);
    }

    let compiled = db
        .post()
        .find_many()
        .filter(db::post::PUBLISHED.eq(true))
        .to_sql()?;
    println!("{}", compiled.sql);

    Ok(())
}
