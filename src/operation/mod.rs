pub mod op_ctx;

// Operations are short lived processes run over the b-tree or what touch memory/pages
// We must define policies for how different operations might run or be executed for example, a Vacuum might need to perform operations which
// a transaction might not be able to do
pub(crate) enum OpType {
    Operation,
    Transaction,
    Vacuum,
    Background,
    Restore,
}
