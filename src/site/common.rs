pub trait Site {
    type ViewType;
    type Slide;
    type Error;

    fn new(cookie_file: &str) -> Self;

    fn get(
        &self,
        t: Self::ViewType,
        limit: Option<u32>,
    ) -> impl std::future::Future<Output = Result<Vec<Self::Slide>, Self::Error>> + Send;
}

pub enum Quality {
    Best,
    Mid,
    Low,
}
