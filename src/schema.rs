diesel::table! {
    users (id) {
        id -> Text,
        email -> Text,
        first_name -> Text,
        last_name -> Text,
        password_hash -> Text,
        token_version -> Integer,
        is_admin -> Bool,
        account_status -> Text,
        must_change_password -> Bool,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    blacklisted_tokens (token_signature_hex) {
        token_signature_hex -> Text,
        token_expiration -> BigInt,
    }
}

diesel::table! {
    posts (id) {
        id -> Text,
        author_user_id -> Nullable<Text>,
        is_anonymous -> Bool,
        approval_status -> Text,
        body -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    replies (id) {
        id -> Text,
        post_id -> Text,
        author_user_id -> Text,
        body -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    user_notifications (id) {
        id -> Text,
        user_id -> Text,
        kind -> Text,
        post_id -> Nullable<Text>,
        reply_id -> Nullable<Text>,
        actor_user_id -> Nullable<Text>,
        message -> Text,
        created_at -> BigInt,
        read_at -> Nullable<BigInt>,
    }
}

diesel::table! {
    upcoming_events (id) {
        id -> Text,
        name -> Text,
        event_date -> Text,
        event_time -> Nullable<Text>,
        end_date -> Nullable<Text>,
        end_time -> Nullable<Text>,
        location -> Nullable<Text>,
        description -> Nullable<Text>,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    study_topics (id) {
        id -> Text,
        week_start -> Text,
        name -> Text,
        description -> Nullable<Text>,
        hyperlink -> Nullable<Text>,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    survey_responses (id) {
        id -> Text,
        food_suggestions -> Nullable<Text>,
        dietary_restrictions -> Nullable<Text>,
        created_at -> BigInt,
    }
}

diesel::joinable!(replies -> posts (post_id));
diesel::joinable!(replies -> users (author_user_id));
diesel::joinable!(user_notifications -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    users,
    blacklisted_tokens,
    posts,
    replies,
    user_notifications,
    upcoming_events,
    study_topics,
    survey_responses,
);
