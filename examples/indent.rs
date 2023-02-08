use chumsky::zero_copy::prelude::*;

#[derive(Clone, Debug)]
enum Stmt {
    Expr,
    Loop(Vec<Stmt>),
}

fn parser<'a>() -> impl Parser<'a, str, Vec<Stmt>> {
    let expr = just("expr"); // TODO

    let block = recursive(|block| {
        let indent = any().filter(|c| *c == ' ')
            .repeated()
            .configure(|cfg, parent_indent| cfg.exactly(*parent_indent));

        let expr_stmt = expr
            .then_ignore(text::newline())
            .to(Stmt::Expr);
        let control_flow = just("loop:")
            .then(text::newline())
            .ignore_then(block).map(Stmt::Loop);
        let stmt = expr_stmt.or(control_flow);

        text::whitespace().count()
            .then_with_ctx(stmt
                .separated_by(indent)
                .collect())
    });

    block.with_ctx(0)
}

fn main() {
    let stmts = parser()
        .padded()
        .then_ignore(end())
        .parse(r#"
expr
expr
loop:
    expr
    loop:
        expr
        expr
    expr
expr
"#);
    println!("{:#?}", stmts.output());
    println!("{:?}", stmts.errors().collect::<Vec<_>>());
}
