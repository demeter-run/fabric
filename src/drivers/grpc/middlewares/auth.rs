#[derive(Clone)]
pub struct Authenticator {}

impl tonic::service::Interceptor for Authenticator {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
    }
}
