table! {
    actions (id) {
        id -> Int4,
        guild -> Int8,
        module -> Text,
        action -> Text,
        in_channel -> Nullable<Int8>,
        message -> Nullable<Text>,
    }
}

table! {
    enabled_modules (guild, module) {
        guild -> Int8,
        module -> Text,
        enabled -> Bool,
    }
}

allow_tables_to_appear_in_same_query!(
    actions,
    enabled_modules,
);
