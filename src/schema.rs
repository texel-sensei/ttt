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
    projects (id) {
        id -> Integer,
        name -> Text,
        archived -> Bool,
        last_access_time -> Text,
    }
}

diesel::table! {
    tags (id) {
        id -> Integer,
        name -> Text,
        archived -> Bool,
        last_access_time -> Text,
    }
}

diesel::table! {
    tags_per_project (project_id, tag_id) {
        project_id -> Integer,
        tag_id -> Integer,
    }
}

diesel::joinable!(frames -> projects (project));
diesel::joinable!(tags_per_project -> projects (project_id));
diesel::joinable!(tags_per_project -> tags (tag_id));

diesel::allow_tables_to_appear_in_same_query!(frames, projects, tags, tags_per_project,);
