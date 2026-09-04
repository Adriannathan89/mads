diesel::table! {
    comments {
        id -> Int8,
        user_id -> Int8,
        body -> Text,
    }
}
