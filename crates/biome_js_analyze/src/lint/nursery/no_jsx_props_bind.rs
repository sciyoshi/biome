use biome_analyze::{
    Ast, Rule, RuleDiagnostic, RuleDomain, RuleSource, context::RuleContext, declare_lint_rule,
};
use biome_console::markup;
use biome_diagnostics::Severity;
use biome_js_syntax::{
    AnyJsExpression, AnyJsxAttribute, AnyJsxAttributeValue, JsCallExpression, JsSyntaxKind,
    JsxAttribute,
};
use biome_rowan::{AstNode, SyntaxNode};

declare_lint_rule! {
    /// Prevents usage of `.bind()` or arrow functions in JSX props.
    ///
    /// Creating new functions inside props can cause performance issues, as it creates a brand new function on every render.
    ///
    /// ## Examples
    ///
    /// ### Invalid
    ///
    /// ```jsx,expect_diagnostic
    /// <Foo onClick={this.handleClick.bind(this)} />
    /// ```
    ///
    /// ```jsx,expect_diagnostic
    /// <Foo onClick={() => console.log('Click')} />
    /// ```
    ///
    /// ### Valid
    ///
    /// ```jsx
    /// <Foo onClick={this.handleClick} />
    /// ```
    ///
    /// ```jsx
    /// <Foo onClick={handleClick} />
    /// ```
    pub NoJsxPropsBind {
        version: "1.0.0",
        name: "noJsxPropsBind",
        language: "jsx",
        sources: &[RuleSource::EslintReact("jsx-no-bind")],
        recommended: true,
        severity: Severity::Warning,
    }
}

impl Rule for NoJsxPropsBind {
    type Query = Ast<JsxAttribute>;
    type State = BindType;
    type Signals = Option<Self::State>;
    type Options = ();

    fn run(ctx: &RuleContext<Self>) -> Self::Signals {
        let attribute = ctx.query();
        let initializer = attribute.initializer()?;

        // Check for value inside the attribute
        let value = initializer.value().ok()?;

        // We're only interested in expression attributes
        let expression = match value {
            AnyJsxAttributeValue::JsxExpressionAttributeValue(expr_attr) => {
                expr_attr.expression().ok()?
            }
            _ => return None,
        };

        // Check if the expression is an arrow function
        if matches!(expression, AnyJsExpression::JsArrowFunctionExpression(_)) {
            return Some(BindType::ArrowFunction(
                expression.syntax().text_trimmed_range(),
            ));
        }

        // Check if the expression is a .bind() call
        if let AnyJsExpression::JsCallExpression(call_expr) = &expression {
            if is_bind_call(call_expr) {
                return Some(BindType::BindCall(call_expr.syntax().text_trimmed_range()));
            }
        }

        None
    }

    fn diagnostic(_: &RuleContext<Self>, state: &Self::State) -> Option<RuleDiagnostic> {
        let (message, range) = match state {
            BindType::BindCall(range) => (
                markup! {
                    "Don't use " <Emphasis>".bind()"</Emphasis> " in JSX props."
                },
                range,
            ),
            BindType::ArrowFunction(range) => (
                markup! {
                    "Don't use " <Emphasis>"arrow functions"</Emphasis> " in JSX props."
                },
                range,
            ),
        };

        Some(
            RuleDiagnostic::new(rule_category!(), *range, message)
                .note(markup! {
                    "Creating functions inside props creates a new function on every render, which may cause unnecessary re-renders."
                })
        )
    }
}

/// The type of binding detected in JSX props
pub enum BindType {
    /// A .bind() call (e.g., `this.handleClick.bind(this)`)
    BindCall(biome_rowan::TextRange),
    /// An arrow function (e.g., `() => this.handleClick()`)
    ArrowFunction(biome_rowan::TextRange),
}

/// Check if a call expression is a `.bind()` call
fn is_bind_call(call_expr: &JsCallExpression) -> bool {
    // Extract the callee
    if let Ok(callee) = call_expr.callee() {
        // Check if the callee is a member expression
        if let AnyJsExpression::JsStaticMemberExpression(member_expr) = callee {
            // Check if the property is 'bind'
            if let Ok(property) = member_expr.member() {
                let name = property.to_trimmed_string();
                return name == "bind";
            }
        }
    }

    false
}
