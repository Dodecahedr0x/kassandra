use litesvm::LiteSVM;

#[test]
fn program_loads() {
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/kassandra_program.so");
    let program_id = solana_sdk::pubkey::Pubkey::new_from_array(kassandra_program::ID.to_bytes());
    svm.add_program(program_id, bytes);
    // Loading without panicking is the assertion.
}
