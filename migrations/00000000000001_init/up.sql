CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    email TEXT NOT NULL UNIQUE,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    token_version INTEGER NOT NULL DEFAULT 0,
    is_admin BOOLEAN NOT NULL DEFAULT 0,
    account_status TEXT NOT NULL,
    must_change_password BOOLEAN NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    CONSTRAINT chk_users_id_length CHECK (length(id) <= 36),
    CONSTRAINT chk_users_email_length CHECK (length(email) <= 254),
    CONSTRAINT chk_users_first_name_length CHECK (length(first_name) <= 100),
    CONSTRAINT chk_users_last_name_length CHECK (length(last_name) <= 100),
    CONSTRAINT chk_users_password_hash_length CHECK (length(password_hash) <= 512),
    CONSTRAINT chk_users_account_status CHECK (
        account_status IN ('pending_approval', 'active', 'suspended', 'locked')
    )
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_status ON users(account_status, created_at DESC);
CREATE INDEX idx_users_admin ON users(is_admin);

CREATE TABLE blacklisted_tokens (
    token_signature_hex TEXT PRIMARY KEY NOT NULL,
    token_expiration BIGINT NOT NULL,
    CONSTRAINT chk_blacklisted_tokens_signature_length CHECK (length(token_signature_hex) <= 64)
);

CREATE INDEX idx_blacklisted_tokens_exp ON blacklisted_tokens(token_expiration);

CREATE TABLE posts (
    id TEXT PRIMARY KEY NOT NULL,
    author_user_id TEXT,
    is_anonymous BOOLEAN NOT NULL DEFAULT 0,
    approval_status TEXT NOT NULL DEFAULT 'approved',
    body TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    CONSTRAINT chk_posts_id_length CHECK (length(id) <= 36),
    CONSTRAINT chk_posts_author_user_id_length CHECK (
        author_user_id IS NULL OR length(author_user_id) <= 36
    ),
    CONSTRAINT chk_posts_body_length CHECK (length(body) <= 100000),
    CONSTRAINT fk_posts_author FOREIGN KEY (author_user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT chk_posts_anonymous_shape CHECK (
        (is_anonymous = 0 AND author_user_id IS NOT NULL)
        OR
        (is_anonymous = 1 AND author_user_id IS NULL)
    ),
    CONSTRAINT chk_posts_approval_status CHECK (
        approval_status IN ('pending_approval', 'approved')
    )
);

CREATE INDEX idx_posts_feed ON posts(created_at DESC);
CREATE INDEX idx_posts_approval_feed ON posts(approval_status, created_at DESC);

CREATE TABLE replies (
    id TEXT PRIMARY KEY NOT NULL,
    post_id TEXT NOT NULL,
    author_user_id TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    CONSTRAINT chk_replies_id_length CHECK (length(id) <= 36),
    CONSTRAINT chk_replies_post_id_length CHECK (length(post_id) <= 36),
    CONSTRAINT chk_replies_author_user_id_length CHECK (length(author_user_id) <= 36),
    CONSTRAINT chk_replies_body_length CHECK (length(body) <= 100000),
    CONSTRAINT fk_replies_post FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE,
    CONSTRAINT fk_replies_author FOREIGN KEY (author_user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_replies_post ON replies(post_id, created_at ASC);
CREATE INDEX idx_replies_author ON replies(author_user_id, created_at DESC);

CREATE TABLE user_notifications (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    post_id TEXT,
    reply_id TEXT,
    actor_user_id TEXT,
    message TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    read_at BIGINT,
    CONSTRAINT chk_user_notifications_id_length CHECK (length(id) <= 36),
    CONSTRAINT chk_user_notifications_user_id_length CHECK (length(user_id) <= 36),
    CONSTRAINT chk_user_notifications_kind_length CHECK (length(kind) <= 50),
    CONSTRAINT chk_user_notifications_post_id_length CHECK (
        post_id IS NULL OR length(post_id) <= 36
    ),
    CONSTRAINT chk_user_notifications_reply_id_length CHECK (
        reply_id IS NULL OR length(reply_id) <= 36
    ),
    CONSTRAINT chk_user_notifications_actor_user_id_length CHECK (
        actor_user_id IS NULL OR length(actor_user_id) <= 36
    ),
    CONSTRAINT chk_user_notifications_message_length CHECK (length(message) <= 1000),
    CONSTRAINT fk_user_notifications_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT fk_user_notifications_post FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE,
    CONSTRAINT fk_user_notifications_reply FOREIGN KEY (reply_id) REFERENCES replies(id) ON DELETE CASCADE,
    CONSTRAINT fk_user_notifications_actor FOREIGN KEY (actor_user_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX idx_user_notifications_user ON user_notifications(user_id, created_at DESC);
CREATE INDEX idx_user_notifications_unread ON user_notifications(user_id, read_at, created_at DESC);

CREATE TABLE upcoming_events (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    event_date TEXT NOT NULL,
    event_time TEXT,
    end_date TEXT,
    end_time TEXT,
    location TEXT,
    description TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    CONSTRAINT chk_upcoming_events_id_length CHECK (length(id) <= 36),
    CONSTRAINT chk_upcoming_events_name_length CHECK (length(name) <= 200),
    CONSTRAINT chk_upcoming_events_event_date_length CHECK (length(event_date) <= 10),
    CONSTRAINT chk_upcoming_events_event_time_length CHECK (
        event_time IS NULL OR length(event_time) <= 5
    ),
    CONSTRAINT chk_upcoming_events_end_date_length CHECK (
        end_date IS NULL OR length(end_date) <= 10
    ),
    CONSTRAINT chk_upcoming_events_end_time_length CHECK (
        end_time IS NULL OR length(end_time) <= 5
    ),
    CONSTRAINT chk_upcoming_events_location_length CHECK (
        location IS NULL OR length(location) <= 255
    ),
    CONSTRAINT chk_upcoming_events_description_length CHECK (
        description IS NULL OR length(description) <= 5000
    )
);

CREATE INDEX idx_upcoming_events_date_time
    ON upcoming_events(event_date ASC, event_time ASC, created_at ASC);

CREATE TABLE study_topics (
    id TEXT PRIMARY KEY NOT NULL,
    week_start TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    hyperlink TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    CONSTRAINT chk_study_topics_id_length CHECK (length(id) <= 36),
    CONSTRAINT chk_study_topics_week_start_length CHECK (length(week_start) <= 10),
    CONSTRAINT chk_study_topics_name_length CHECK (length(name) <= 200),
    CONSTRAINT chk_study_topics_description_length CHECK (
        description IS NULL OR length(description) <= 5000
    ),
    CONSTRAINT chk_study_topics_hyperlink_length CHECK (
        hyperlink IS NULL OR length(hyperlink) <= 2048
    )
);

CREATE INDEX idx_study_topics_week_start ON study_topics(week_start ASC);

CREATE TABLE survey_responses (
    id TEXT PRIMARY KEY NOT NULL,
    food_suggestions TEXT,
    dietary_restrictions TEXT,
    created_at BIGINT NOT NULL,
    CONSTRAINT chk_survey_responses_id_length CHECK (length(id) <= 36),
    CONSTRAINT chk_survey_responses_food_suggestions_length CHECK (
        food_suggestions IS NULL OR length(food_suggestions) <= 512
    ),
    CONSTRAINT chk_survey_responses_dietary_restrictions_length CHECK (
        dietary_restrictions IS NULL OR length(dietary_restrictions) <= 512
    )
);

CREATE INDEX idx_survey_responses_created_at
    ON survey_responses(created_at DESC);
