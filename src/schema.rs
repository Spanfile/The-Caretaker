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

    guild_settings (guild) {
        guild -> Int8,
        admin_role -> Nullable<Int8>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::module::dbimport::*;

    module_exclusions (guild, module, kind, id) {
        guild -> Int8,
        module -> Module_kind,
        kind -> Exclusion_kind,
        id -> Int8,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::module::dbimport::*;

    module_settings (guild, module, setting) {
        guild -> Int8,
        module -> Module_kind,
        setting -> Text,
        value -> Text,
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
    guild_settings,
    module_exclusions,
    module_settings,
    modules,
);
