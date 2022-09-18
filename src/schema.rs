// @generated automatically by Diesel CLI.

diesel::table! {
    frames (id) {
        id -> Integer,
        project -> Integer,
        start -> Text,
        end -> Nullable<Text>,
    }
}

diesel::table! {
    posts (id) {
        id -> Integer,
        title -> Text,
        body -> Text,
        published -> Bool,
    }
}

diesel::table! {
    projects (id) {
        id -> Integer,
        name -> Text,
        archived -> Bool,
        last_access_time -> Text,
    }
}

diesel::joinable!(frames -> projects (project));

diesel::allow_tables_to_appear_in_same_query!(frames, posts, projects,);
