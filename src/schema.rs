table! {
    enabled_modules (guild, module) {
        guild -> Int8,
        module -> Text,
        enabled -> Bool,
    }
}
