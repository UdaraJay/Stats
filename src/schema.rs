// @generated automatically by Diesel CLI.

diesel::table! {
    collectors (id) {
        id -> Text,
        origin -> Text,
        country -> Text,
        city -> Text,
        timestamp -> Timestamp,
    }
}

diesel::table! {
    events (id) {
        id -> Text,
        url -> Text,
        name -> Text,
        timestamp -> Timestamp,
        collector_id -> Text,
    }
}

diesel::allow_tables_to_appear_in_same_query!(collectors, events,);
