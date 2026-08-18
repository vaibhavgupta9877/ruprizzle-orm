mod db;

#[tokio::main]
async fn main() -> Result<(), ruprizzle::Error> {
    dotenvy::dotenv().ok();
    let db = db::Db::connect(&std::env::var("DATABASE_URL")?).await?;

    let mut tx = db.raw_pool().begin().await?;

    let author = db
        .user()
        .create(db::UserInsert {
            id: None,
            email: "alice@example.com".into(),
            name: Some("Alice".into()),
            role: Some(db::Role::ADMIN),
        })
        .exec(&mut tx)
        .await?;

    let _post = db
        .post()
        .create(db::PostInsert {
            id: None,
            title: "Hello, ruprizzle".into(),
            body: Some("This is the first post.".into()),
            published: Some(true),
            author_id: Some(author.id),
        })
        .exec(&mut tx)
        .await?;

    tx.commit().await?;

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
            .and_then(|a| a.name.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        println!("{} by {}", post.title, author_name);
    }

    let sql = db
        .post()
        .find_many()
        .filter(db::post::PUBLISHED.eq(true))
        .to_sql();
    println!("{sql}");

    Ok(())
}
