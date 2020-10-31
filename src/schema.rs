table! {
    use diesel::sql_types::*;
    use crate::module::dbimport::*;

    actions (id) {
        id -> Int4,
        guild -> Int8,
        module -> Module_kind,
        action -> Action_kind,
        in_channel -> Nullable<Int8>,
        message -> Nullable<Text>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::module::dbimport::*;

    modules (guild, module) {
        guild -> Int8,
        module -> Module_kind,
        enabled -> Bool,
    }
}

allow_tables_to_appear_in_same_query!(
    actions,
    modules,
);
