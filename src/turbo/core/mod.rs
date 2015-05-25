pub use self::shell::{Shell, MultiShell, ShellConfig};
pub mod shell;

#[macro_export]
macro_rules! stick_cmds {
    ($name:ident $expr:expr) => (

        fn $name() {
            $expr;
        }

        
    )

}
