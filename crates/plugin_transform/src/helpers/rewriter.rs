use std::collections::HashMap;

use oxc::allocator::Allocator;
use oxc::ast::ast::AccessorProperty;
use oxc::ast::ast::ArrayAssignmentTarget;
use oxc::ast::ast::ArrayExpression;
use oxc::ast::ast::ArrayPattern;
use oxc::ast::ast::ArrowFunctionExpression;
use oxc::ast::ast::AssignmentExpression;
use oxc::ast::ast::AssignmentPattern;
use oxc::ast::ast::AssignmentTargetPropertyIdentifier;
use oxc::ast::ast::AssignmentTargetPropertyProperty;
use oxc::ast::ast::AssignmentTargetRest;
use oxc::ast::ast::AssignmentTargetWithDefault;
use oxc::ast::ast::AwaitExpression;
use oxc::ast::ast::BigIntLiteral;
use oxc::ast::ast::BinaryExpression;
use oxc::ast::ast::BindingIdentifier;
use oxc::ast::ast::BindingProperty;
use oxc::ast::ast::BindingRestElement;
use oxc::ast::ast::BlockStatement;
use oxc::ast::ast::BooleanLiteral;
use oxc::ast::ast::BreakStatement;
use oxc::ast::ast::CallExpression;
use oxc::ast::ast::CatchClause;
use oxc::ast::ast::CatchParameter;
use oxc::ast::ast::ChainExpression;
use oxc::ast::ast::Class;
use oxc::ast::ast::ClassBody;
use oxc::ast::ast::ComputedMemberExpression;
use oxc::ast::ast::ConditionalExpression;
use oxc::ast::ast::ContinueStatement;
use oxc::ast::ast::DebuggerStatement;
use oxc::ast::ast::Directive;
use oxc::ast::ast::DoWhileStatement;
use oxc::ast::ast::Elision;
use oxc::ast::ast::EmptyStatement;
use oxc::ast::ast::ExportAllDeclaration;
use oxc::ast::ast::ExportDeclaration;
use oxc::ast::ast::ExportDefaultDeclaration;
use oxc::ast::ast::ExportFromDeclaration;
use oxc::ast::ast::ExportNamedDeclaration;
use oxc::ast::ast::ExportSpecifier;
use oxc::ast::ast::ExpressionStatement;
use oxc::ast::ast::ForInStatement;
use oxc::ast::ast::ForOfStatement;
use oxc::ast::ast::ForStatement;
use oxc::ast::ast::FormalParameter;
use oxc::ast::ast::FormalParameterRest;
use oxc::ast::ast::FormalParameters;
use oxc::ast::ast::Function;
use oxc::ast::ast::FunctionBody;
use oxc::ast::ast::Hashbang;
use oxc::ast::ast::IdentifierName;
use oxc::ast::ast::IdentifierReference;
use oxc::ast::ast::IfStatement;
use oxc::ast::ast::ImportAttribute;
use oxc::ast::ast::ImportDeclaration;
use oxc::ast::ast::ImportDefaultSpecifier;
use oxc::ast::ast::ImportExpression;
use oxc::ast::ast::ImportMeta;
use oxc::ast::ast::ImportNamespaceSpecifier;
use oxc::ast::ast::ImportSpecifier;
use oxc::ast::ast::LabelIdentifier;
use oxc::ast::ast::LabeledStatement;
use oxc::ast::ast::LogicalExpression;
use oxc::ast::ast::MethodDefinition;
use oxc::ast::ast::NewExpression;
use oxc::ast::ast::NewTarget;
use oxc::ast::ast::NullLiteral;
use oxc::ast::ast::NumericLiteral;
use oxc::ast::ast::ObjectAssignmentTarget;
use oxc::ast::ast::ObjectExpression;
use oxc::ast::ast::ObjectPattern;
use oxc::ast::ast::ObjectProperty;
use oxc::ast::ast::ParenthesizedExpression;
use oxc::ast::ast::PrivateFieldExpression;
use oxc::ast::ast::PrivateIdentifier;
use oxc::ast::ast::PrivateInExpression;
use oxc::ast::ast::PropertyDefinition;
use oxc::ast::ast::RegExpLiteral;
use oxc::ast::ast::ReturnStatement;
use oxc::ast::ast::SequenceExpression;
use oxc::ast::ast::SpreadElement;
use oxc::ast::ast::StaticBlock;
use oxc::ast::ast::StaticMemberExpression;
use oxc::ast::ast::StringLiteral;
use oxc::ast::ast::Super;
use oxc::ast::ast::SwitchCase;
use oxc::ast::ast::SwitchStatement;
use oxc::ast::ast::TaggedTemplateExpression;
use oxc::ast::ast::TemplateElement;
use oxc::ast::ast::TemplateLiteral;
use oxc::ast::ast::ThisExpression;
use oxc::ast::ast::ThrowStatement;
use oxc::ast::ast::TryStatement;
use oxc::ast::ast::UnaryExpression;
use oxc::ast::ast::UpdateExpression;
use oxc::ast::ast::V8IntrinsicExpression;
use oxc::ast::ast::VariableDeclaration;
use oxc::ast::ast::VariableDeclarator;
use oxc::ast::ast::WhileStatement;
use oxc::ast::ast::WithClause;
use oxc::ast::ast::WithStatement;
use oxc::ast::ast::YieldExpression;
use oxc::ast_visit::VisitMut;
use oxc::semantic::Scoping;
use oxc::span::SPAN;
use oxc::str::Ident;
use oxc::syntax::reference::Reference;
use oxc::syntax::scope::ScopeFlags;
use oxc::syntax::symbol::SymbolId;

/// Implement [`VisitMut`] methods that reset the visited node's span before
/// walking its children (only when [`Rewriter::reset_spans`] is set).
macro_rules! visit_span_resets {
    ($( $method:ident, $walk:ident, $ty:ty );* $(;)?) => {
        $(
            fn $method(&mut self, it: &mut $ty) {
                if self.reset_spans {
                    it.span = SPAN;
                }

                oxc::ast_visit::walk_mut::$walk(self, it);
            }
        )*
    };
}

/// Renames identifiers bound to a set of symbols and optionally resets every
/// span it visits to [`SPAN`].
///
/// Renaming is symbol-based: identifier references are resolved through the
/// scoping of the AST being rewritten, so only references that actually bind
/// to a renamed top-level symbol change — nested shadowing is untouched.
pub struct Rewriter<'a, 's> {
    pub allocator: &'a Allocator,
    /// Scoping of the AST being rewritten (helper source or main program).
    pub scoping: &'s Scoping,
    /// Symbol ID to final binding name.
    pub renames: HashMap<SymbolId, String>,
    /// Whether spans must be reset to [`SPAN`] (for spliced helper code).
    pub reset_spans: bool,
}

impl<'a, 's> VisitMut<'a> for Rewriter<'a, 's> {
    fn visit_function(
        &mut self,
        it: &mut Function<'a>,
        flags: ScopeFlags,
    ) {
        if self.reset_spans {
            it.span = SPAN;
        }

        oxc::ast_visit::walk_mut::walk_function(self, it, flags);
    }

    fn visit_identifier_reference(
        &mut self,
        it: &mut IdentifierReference<'a>,
    ) {
        if let Some(reference_id) = it.reference_id.get() {
            let reference: &Reference =
                self.scoping.get_reference(reference_id);

            if let Some(symbol_id) = reference.symbol_id()
                && let Some(final_name) = self.renames.get(&symbol_id)
            {
                it.name = Ident::from_str_in(final_name, &self.allocator);
            }
        }

        if self.reset_spans {
            it.span = SPAN;
        }
    }

    fn visit_binding_identifier(
        &mut self,
        it: &mut BindingIdentifier<'a>,
    ) {
        if let Some(symbol_id) = it.symbol_id.get()
            && let Some(final_name) = self.renames.get(&symbol_id)
        {
            it.name = Ident::from_str_in(final_name, &self.allocator);
        }

        if self.reset_spans {
            it.span = SPAN;
        }
    }

    visit_span_resets! {
        visit_accessor_property, walk_accessor_property, AccessorProperty<'a>;
        visit_array_assignment_target, walk_array_assignment_target, ArrayAssignmentTarget<'a>;
        visit_array_expression, walk_array_expression, ArrayExpression<'a>;
        visit_array_pattern, walk_array_pattern, ArrayPattern<'a>;
        visit_arrow_function_expression, walk_arrow_function_expression, ArrowFunctionExpression<'a>;
        visit_assignment_expression, walk_assignment_expression, AssignmentExpression<'a>;
        visit_assignment_pattern, walk_assignment_pattern, AssignmentPattern<'a>;
        visit_assignment_target_property_identifier, walk_assignment_target_property_identifier, AssignmentTargetPropertyIdentifier<'a>;
        visit_assignment_target_property_property, walk_assignment_target_property_property, AssignmentTargetPropertyProperty<'a>;
        visit_assignment_target_rest, walk_assignment_target_rest, AssignmentTargetRest<'a>;
        visit_assignment_target_with_default, walk_assignment_target_with_default, AssignmentTargetWithDefault<'a>;
        visit_await_expression, walk_await_expression, AwaitExpression<'a>;
        visit_big_int_literal, walk_big_int_literal, BigIntLiteral<'a>;
        visit_binary_expression, walk_binary_expression, BinaryExpression<'a>;
        visit_binding_property, walk_binding_property, BindingProperty<'a>;
        visit_binding_rest_element, walk_binding_rest_element, BindingRestElement<'a>;
        visit_block_statement, walk_block_statement, BlockStatement<'a>;
        visit_boolean_literal, walk_boolean_literal, BooleanLiteral;
        visit_break_statement, walk_break_statement, BreakStatement<'a>;
        visit_call_expression, walk_call_expression, CallExpression<'a>;
        visit_catch_clause, walk_catch_clause, CatchClause<'a>;
        visit_catch_parameter, walk_catch_parameter, CatchParameter<'a>;
        visit_chain_expression, walk_chain_expression, ChainExpression<'a>;
        visit_class, walk_class, Class<'a>;
        visit_class_body, walk_class_body, ClassBody<'a>;
        visit_computed_member_expression, walk_computed_member_expression, ComputedMemberExpression<'a>;
        visit_conditional_expression, walk_conditional_expression, ConditionalExpression<'a>;
        visit_continue_statement, walk_continue_statement, ContinueStatement<'a>;
        visit_debugger_statement, walk_debugger_statement, DebuggerStatement;
        visit_directive, walk_directive, Directive<'a>;
        visit_do_while_statement, walk_do_while_statement, DoWhileStatement<'a>;
        visit_elision, walk_elision, Elision;
        visit_empty_statement, walk_empty_statement, EmptyStatement;
        visit_export_all_declaration, walk_export_all_declaration, ExportAllDeclaration<'a>;
        visit_export_declaration, walk_export_declaration, ExportDeclaration<'a>;
        visit_export_default_declaration, walk_export_default_declaration, ExportDefaultDeclaration<'a>;
        visit_export_from_declaration, walk_export_from_declaration, ExportFromDeclaration<'a>;
        visit_export_named_declaration, walk_export_named_declaration, ExportNamedDeclaration<'a>;
        visit_export_specifier, walk_export_specifier, ExportSpecifier<'a>;
        visit_expression_statement, walk_expression_statement, ExpressionStatement<'a>;
        visit_for_in_statement, walk_for_in_statement, ForInStatement<'a>;
        visit_for_of_statement, walk_for_of_statement, ForOfStatement<'a>;
        visit_for_statement, walk_for_statement, ForStatement<'a>;
        visit_formal_parameter, walk_formal_parameter, FormalParameter<'a>;
        visit_formal_parameter_rest, walk_formal_parameter_rest, FormalParameterRest<'a>;
        visit_formal_parameters, walk_formal_parameters, FormalParameters<'a>;
        visit_function_body, walk_function_body, FunctionBody<'a>;
        visit_hashbang, walk_hashbang, Hashbang<'a>;
        visit_identifier_name, walk_identifier_name, IdentifierName<'a>;
        visit_if_statement, walk_if_statement, IfStatement<'a>;
        visit_import_attribute, walk_import_attribute, ImportAttribute<'a>;
        visit_import_declaration, walk_import_declaration, ImportDeclaration<'a>;
        visit_import_default_specifier, walk_import_default_specifier, ImportDefaultSpecifier<'a>;
        visit_import_expression, walk_import_expression, ImportExpression<'a>;
        visit_import_meta, walk_import_meta, ImportMeta;
        visit_import_namespace_specifier, walk_import_namespace_specifier, ImportNamespaceSpecifier<'a>;
        visit_import_specifier, walk_import_specifier, ImportSpecifier<'a>;
        visit_label_identifier, walk_label_identifier, LabelIdentifier<'a>;
        visit_labeled_statement, walk_labeled_statement, LabeledStatement<'a>;
        visit_logical_expression, walk_logical_expression, LogicalExpression<'a>;
        visit_method_definition, walk_method_definition, MethodDefinition<'a>;
        visit_new_expression, walk_new_expression, NewExpression<'a>;
        visit_new_target, walk_new_target, NewTarget;
        visit_null_literal, walk_null_literal, NullLiteral;
        visit_numeric_literal, walk_numeric_literal, NumericLiteral<'a>;
        visit_object_assignment_target, walk_object_assignment_target, ObjectAssignmentTarget<'a>;
        visit_object_expression, walk_object_expression, ObjectExpression<'a>;
        visit_object_pattern, walk_object_pattern, ObjectPattern<'a>;
        visit_object_property, walk_object_property, ObjectProperty<'a>;
        visit_parenthesized_expression, walk_parenthesized_expression, ParenthesizedExpression<'a>;
        visit_private_field_expression, walk_private_field_expression, PrivateFieldExpression<'a>;
        visit_private_identifier, walk_private_identifier, PrivateIdentifier<'a>;
        visit_private_in_expression, walk_private_in_expression, PrivateInExpression<'a>;
        visit_property_definition, walk_property_definition, PropertyDefinition<'a>;
        visit_reg_exp_literal, walk_reg_exp_literal, RegExpLiteral<'a>;
        visit_return_statement, walk_return_statement, ReturnStatement<'a>;
        visit_sequence_expression, walk_sequence_expression, SequenceExpression<'a>;
        visit_spread_element, walk_spread_element, SpreadElement<'a>;
        visit_static_block, walk_static_block, StaticBlock<'a>;
        visit_static_member_expression, walk_static_member_expression, StaticMemberExpression<'a>;
        visit_string_literal, walk_string_literal, StringLiteral<'a>;
        visit_super, walk_super, Super;
        visit_switch_case, walk_switch_case, SwitchCase<'a>;
        visit_switch_statement, walk_switch_statement, SwitchStatement<'a>;
        visit_tagged_template_expression, walk_tagged_template_expression, TaggedTemplateExpression<'a>;
        visit_template_element, walk_template_element, TemplateElement<'a>;
        visit_template_literal, walk_template_literal, TemplateLiteral<'a>;
        visit_this_expression, walk_this_expression, ThisExpression;
        visit_throw_statement, walk_throw_statement, ThrowStatement<'a>;
        visit_try_statement, walk_try_statement, TryStatement<'a>;
        visit_unary_expression, walk_unary_expression, UnaryExpression<'a>;
        visit_update_expression, walk_update_expression, UpdateExpression<'a>;
        visit_v8_intrinsic_expression, walk_v8_intrinsic_expression, V8IntrinsicExpression<'a>;
        visit_variable_declaration, walk_variable_declaration, VariableDeclaration<'a>;
        visit_variable_declarator, walk_variable_declarator, VariableDeclarator<'a>;
        visit_while_statement, walk_while_statement, WhileStatement<'a>;
        visit_with_clause, walk_with_clause, WithClause<'a>;
        visit_with_statement, walk_with_statement, WithStatement<'a>;
        visit_yield_expression, walk_yield_expression, YieldExpression<'a>;
    }
}
