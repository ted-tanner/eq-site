use uuid::Uuid;

use crate::auth::{self, TokenClaims};
use crate::db::auth::{AuthDao, ChangePasswordInput, CreateUserInput};
use crate::db::user::UserDao;
use crate::db::{self, DbPool};
use crate::models::user::{ACCOUNT_ACTIVE, ACCOUNT_PENDING, User};
use crate::services::{ServiceError, block_dao};
use crate::utils::temp_password::generate_temp_password;

#[derive(Clone)]
pub struct SignUpInput {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub password: String,
}

#[derive(Clone)]
pub struct ChangePasswordServiceInput {
    pub user_id: String,
    pub current_password: String,
    pub new_password: String,
}

#[derive(Clone)]
pub struct SessionTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub csrf_token: String,
}

#[derive(Clone)]
pub struct SignInOnlyToken {
    pub sign_in_token: String,
    pub csrf_token: String,
}

#[derive(Clone)]
pub struct AccessTokenSession {
    pub user_id: String,
    pub account_status: String,
}

#[derive(Clone)]
pub enum LoginSession {
    Full { user: User, tokens: SessionTokens },
    PasswordChangeRequired { user: User, token: SignInOnlyToken },
}

pub struct AuthService {
    db_pool: DbPool,
}

impl AuthService {
    pub fn new(db_pool: &DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
        }
    }

    pub async fn load_user(&self, user_id: String) -> Result<User, ServiceError> {
        let pool = self.db_pool.clone();
        block_dao(move || UserDao::new(&pool).load_user(&user_id)).await
    }

    pub async fn load_user_by_email(&self, email: String) -> Result<User, ServiceError> {
        let pool = self.db_pool.clone();
        block_dao(move || UserDao::new(&pool).load_user_by_email(&email)).await
    }

    pub async fn sign_up(&self, input: SignUpInput) -> Result<(User, SessionTokens), ServiceError> {
        let email = normalize_email(&input.email);
        let first_name = required_trimmed(&input.first_name, "First name")?;
        let last_name = required_trimmed(&input.last_name, "Last name")?;
        let password = validated_password(&input.password, "Password")?;
        if email.is_empty() {
            return Err(ServiceError::bad_request(
                "First name, last name, email, and password are required",
            ));
        }

        let password_hash = auth::hash_password_on_rayon(password).await?;
        let is_bootstrap_admin = email == crate::env::CONF.bootstrap_admin_email;
        let status = if is_bootstrap_admin {
            ACCOUNT_ACTIVE
        } else {
            ACCOUNT_PENDING
        };
        let pool = self.db_pool.clone();
        let user_id = Uuid::now_v7().to_string();
        let now = db::now_ts();
        let create_input = CreateUserInput {
            id: user_id.clone(),
            email,
            first_name,
            last_name,
            password_hash,
            is_admin: is_bootstrap_admin,
            account_status: status.to_string(),
            now,
        };
        let user = block_dao(move || AuthDao::new(&pool).create_user(create_input))
            .await
            .map_err(|error| match error {
                ServiceError::BadRequest(message) if message == "User already exists" => {
                    ServiceError::conflict("User already exists")
                }
                other => other,
            })?;

        if !is_bootstrap_admin {
            let pool = self.db_pool.clone();
            let pending_user = user.clone();
            block_dao(move || AuthDao::new(&pool).notify_admins_of_pending_user(&pending_user))
                .await?;
        }
        let tokens = self.create_session_tokens(&user)?;
        Ok((user, tokens))
    }

    pub async fn sign_in(
        &self,
        email: String,
        password: String,
    ) -> Result<LoginSession, ServiceError> {
        let email = normalize_email(&email);
        let password = validated_password(&password, "Password")?;

        let user = match self.load_user_by_email(email).await {
            Ok(user) => user,
            Err(ServiceError::NotFound(_)) => {
                let _ = auth::hash_password_on_rayon(password).await?;
                return invalid_login();
            }
            Err(error) => return Err(error),
        };

        let is_valid = auth::verify_password_on_rayon(password, user.password_hash.clone()).await?;
        if !is_valid {
            return invalid_login();
        }

        if user.must_change_password {
            return Ok(LoginSession::PasswordChangeRequired {
                token: self.create_signin_only_token(&user)?,
                user,
            });
        }

        Ok(LoginSession::Full {
            tokens: self.create_session_tokens(&user)?,
            user,
        })
    }

    pub async fn session_from_tokens(
        &self,
        access_token: Option<String>,
        sign_in_token: Option<String>,
    ) -> Result<(User, &'static str), ServiceError> {
        if let Some(token) = access_token
            && let Ok(claims) = auth::verify_access_token(&token)
            && let Ok(user) = self
                .load_valid_session_user(claims.user_id.clone(), claims.token_version)
                .await
        {
            let session_kind = if claims.account_status == ACCOUNT_PENDING {
                "signed_up"
            } else {
                "full"
            };
            return Ok((user, session_kind));
        }

        if let Some(token) = sign_in_token
            && let Ok(claims) = auth::verify_signin_token(&token)
            && let Ok(user) = self
                .load_valid_session_user(claims.user_id.clone(), claims.token_version)
                .await
            && user.must_change_password
        {
            return Ok((user, "password_change_required"));
        }

        Err(ServiceError::unauthorized("Not signed in"))
    }

    pub async fn refresh_tokens(
        &self,
        refresh_token: String,
    ) -> Result<LoginSession, ServiceError> {
        let claims = auth::verify_refresh_token(&refresh_token)
            .map_err(|_| ServiceError::unauthorized("Invalid refresh token"))?;
        let signature_hex = auth::signature_hex_from_token(&refresh_token)?;
        let user = self
            .load_valid_session_user(claims.user_id.clone(), claims.token_version)
            .await
            .map_err(|_| ServiceError::unauthorized("Invalid refresh token"))?;

        let pool = self.db_pool.clone();
        let signature_for_check = signature_hex.clone();
        let blacklisted = block_dao(move || {
            AuthDao::new(&pool).is_refresh_signature_blacklisted(&signature_for_check)
        })
        .await?;
        if blacklisted {
            return Err(ServiceError::unauthorized("Refresh token has been revoked"));
        }

        let pool = self.db_pool.clone();
        block_dao(move || {
            AuthDao::new(&pool)
                .blacklist_refresh_signature(&signature_hex, claims.expiration as i64)
        })
        .await?;
        Ok(LoginSession::Full {
            tokens: self.create_session_tokens(&user)?,
            user,
        })
    }

    pub async fn logout(&self, refresh_token: Option<String>) -> Result<(), ServiceError> {
        self.blacklist_refresh_cookie_if_valid(refresh_token).await
    }

    pub async fn change_password(
        &self,
        input: ChangePasswordServiceInput,
    ) -> Result<LoginSession, ServiceError> {
        let user = self.load_user(input.user_id.clone()).await?;
        let current_password = validated_password(&input.current_password, "Current password")?;
        let new_password = validated_password(&input.new_password, "New password")?;
        let valid =
            auth::verify_password_on_rayon(current_password, user.password_hash.clone()).await?;
        if !valid {
            return Err(ServiceError::unauthorized("Current password is incorrect"));
        }
        let password_hash = auth::hash_password_on_rayon(new_password).await?;
        let pool = self.db_pool.clone();
        let dao_input = ChangePasswordInput {
            user_id: user.id,
            current_token_version: user.token_version,
            password_hash,
            must_change_password: false,
            now: db::now_ts(),
        };
        let updated_user =
            block_dao(move || AuthDao::new(&pool).update_password(dao_input)).await?;
        Ok(LoginSession::Full {
            tokens: self.create_session_tokens(&updated_user)?,
            user: updated_user,
        })
    }

    pub async fn delete_own_account(
        &self,
        user_id: String,
        current_password: String,
        refresh_token: Option<String>,
    ) -> Result<(), ServiceError> {
        let user = self.load_user(user_id).await?;
        let current_password = validated_password(&current_password, "Current password")?;
        let valid =
            auth::verify_password_on_rayon(current_password, user.password_hash.clone()).await?;
        if !valid {
            return Err(ServiceError::unauthorized("Current password is incorrect"));
        }
        if user.is_admin {
            let pool = self.db_pool.clone();
            let admin_count = block_dao(move || AuthDao::new(&pool).admin_count()).await?;
            if admin_count <= 1 {
                return Err(ServiceError::conflict(
                    "The final admin cannot delete their own account",
                ));
            }
        }
        self.blacklist_refresh_cookie_if_valid(refresh_token)
            .await?;
        let pool = self.db_pool.clone();
        let delete_id = user.id;
        block_dao(move || AuthDao::new(&pool).delete_user(&delete_id)).await
    }

    pub async fn admin_reset_password(&self, user_id: String) -> Result<String, ServiceError> {
        let user = self.load_user(user_id).await?;
        let temp_password = generate_temp_password(crate::env::CONF.temp_password_length);
        let password_hash = auth::hash_password_on_rayon(temp_password.clone()).await?;
        let pool = self.db_pool.clone();
        let input = ChangePasswordInput {
            user_id: user.id,
            current_token_version: user.token_version,
            password_hash,
            must_change_password: true,
            now: db::now_ts(),
        };
        block_dao(move || AuthDao::new(&pool).update_password(input)).await?;
        Ok(temp_password)
    }

    pub async fn validate_access_token(
        &self,
        token: String,
    ) -> Result<AccessTokenSession, ServiceError> {
        let claims = auth::verify_access_token(&token)
            .map_err(|_| ServiceError::unauthorized("Invalid access token"))?;
        self.load_valid_session_user(claims.user_id.clone(), claims.token_version)
            .await
            .map_err(|_| ServiceError::unauthorized("Invalid access token"))?;
        Ok(AccessTokenSession {
            user_id: claims.user_id,
            account_status: claims.account_status,
        })
    }

    pub async fn validate_signin_token(&self, token: String) -> Result<String, ServiceError> {
        let claims = auth::verify_signin_token(&token)
            .map_err(|_| ServiceError::unauthorized("Invalid sign-in token"))?;
        let user = self
            .load_valid_session_user(claims.user_id.clone(), claims.token_version)
            .await
            .map_err(|_| ServiceError::unauthorized("Invalid sign-in token"))?;
        if !user.must_change_password {
            return Err(ServiceError::unauthorized("Invalid sign-in token"));
        }
        Ok(claims.user_id)
    }

    async fn load_valid_session_user(
        &self,
        user_id: String,
        token_version: i32,
    ) -> Result<User, ServiceError> {
        let pool = self.db_pool.clone();
        block_dao(move || UserDao::new(&pool).load_session_user(&user_id, token_version)).await
    }

    async fn blacklist_refresh_cookie_if_valid(
        &self,
        refresh_token: Option<String>,
    ) -> Result<(), ServiceError> {
        let Some(token) = refresh_token else {
            return Ok(());
        };
        let Ok(claims) = auth::verify_refresh_token(&token) else {
            return Ok(());
        };
        let signature_hex = auth::signature_hex_from_token(&token)?;
        let pool = self.db_pool.clone();
        block_dao(move || {
            AuthDao::new(&pool)
                .blacklist_refresh_signature_if_absent(&signature_hex, claims.expiration as i64)
        })
        .await
    }

    fn create_session_tokens(&self, user: &User) -> Result<SessionTokens, ServiceError> {
        Ok(SessionTokens {
            access_token: auth::create_access_token(
                &user.id,
                user.token_version,
                &user.account_status,
            )?,
            refresh_token: auth::create_refresh_token(
                &user.id,
                user.token_version,
                &user.account_status,
            )?,
            csrf_token: auth::generate_csrf_token(),
        })
    }

    fn create_signin_only_token(&self, user: &User) -> Result<SignInOnlyToken, ServiceError> {
        Ok(SignInOnlyToken {
            sign_in_token: auth::create_signin_token(
                &user.id,
                user.token_version,
                &user.account_status,
            )?,
            csrf_token: auth::generate_csrf_token(),
        })
    }
}

pub fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

pub fn validated_password(raw: &str, label: &str) -> Result<String, ServiceError> {
    let password = raw.to_string();
    if password.is_empty() {
        return Err(ServiceError::bad_request(format!("{label} is required")));
    }
    if password.len() > crate::env::CONF.max_password_length {
        return Err(ServiceError::bad_request(format!("{label} too long")));
    }
    Ok(password)
}

fn invalid_login<T>() -> Result<T, ServiceError> {
    Err(ServiceError::unauthorized("Incorrect email or password"))
}

fn required_trimmed(value: &str, field: &str) -> Result<String, ServiceError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ServiceError::bad_request(format!("{field} is required")));
    }
    Ok(value.to_string())
}

#[allow(dead_code)]
pub fn claims_from_access_token(token: &str) -> Result<TokenClaims, ServiceError> {
    auth::verify_access_token(token).map_err(|_| ServiceError::unauthorized("Invalid access token"))
}
