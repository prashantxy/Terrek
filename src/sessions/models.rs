pub struct Session{
    pub id : String,
}

pub struct CommandRecord{
    pub Command : String,
    pub Output : String,
}

pub struct UserSessionCommandRecords{
    pub id : String,
    pub user_session_id : String,
    pub timestamp : i64
}

pub struct UserSessionRecords{
    pub id : String,
    pub usersessionrecord : String,
    pub timestamp : i64
}