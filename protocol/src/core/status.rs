//! Status codes das respostas — semelhante ao HTTP Status Code.

/// Status da resposta. O valor numérico é o que trafega no header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok = 200,
    Created = 201,
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    Conflict = 409,
    InternalServerError = 500,
}

impl Status {
    /// Código numérico (200, 404...).
    pub fn code(self) -> u16 {
        self as u16
    }

    /// Razão legível semelhante ao usado no HTTP.
    pub fn reason(self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::Created => "CREATED",
            Status::BadRequest => "BAD REQUEST",
            Status::Unauthorized => "UNAUTHORIZED",
            Status::Forbidden => "FORBIDDEN",
            Status::NotFound => "NOT FOUND",
            Status::Conflict => "CONFLICT",
            Status::InternalServerError => "INTERNAL SERVER ERROR",
        }
    }
}

impl TryFrom<u16> for Status {
    type Error = u16;

    fn try_from(code: u16) -> Result<Self, Self::Error> {
        match code {
            200 => Ok(Status::Ok),
            201 => Ok(Status::Created),
            400 => Ok(Status::BadRequest),
            401 => Ok(Status::Unauthorized),
            403 => Ok(Status::Forbidden),
            404 => Ok(Status::NotFound),
            409 => Ok(Status::Conflict),
            500 => Ok(Status::InternalServerError),
            other => Err(other),
        }
    }
}