use n3_parser::ast;

impl Compile for ast::Model {
    type Args = <ast::ModelInner as Compile>::Args;
    type Output = <ast::ModelInner as Compile>::Output;

    fn compile(self, args: Self::Args) -> Self::Output {
        self.inner.compile(args);
    }
}

impl Compile for ast::ModelInner {
    type Args = ();
    type Output = ();

    fn compile(self, args: Self::Args) -> Self::Output {}
}

pub trait Compile {
    type Args;
    type Output;

    fn compile(self, args: Self::Args) -> Self::Output;
}

#[test]
fn test_lenet_model() {
    const MODEL: &str = "
[LeNet]
  [Conv2d]
    * kernel size = 5
    * stride = 2

  #0 Input RGB image = 1, 28, 28

  #1 Conv2d + ReLU = 32, 14, 14
  #2 Conv2d + ReLU = 64, 7, 7
  #3 Transform = 64 * 7 * 7
  #4 Linear + Sigmoid = 10
";

    let source = n3_parser::parser::parse_file(MODEL).unwrap();
    source.compile(());
}
