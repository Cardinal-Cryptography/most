use drink::session::Session;

#[drink::test]
fn dummy_test(mut session: Session) {
    assert!(true, "This is a dummy test")
}
