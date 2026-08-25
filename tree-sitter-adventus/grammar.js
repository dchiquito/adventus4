/**
 * @file Adventus grammar for tree-sitter
 * @author Daniel Chiquito <daniel.chiquito@gmail.com>
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

export default grammar({
  name: "adventus",

  rules: {
    source_file: $ => repeat($.def),

    keyword_def: $ => "def",
    keyword_if: $ => "if",
    keyword_else: $ => "else",
    keyword_malloc: $ => "malloc",
    keyword_with: $ => "with",
    symbol_lbracket: $ => "[",
    symbol_rbracket: $ => "]",
    symbol_lparen: $ => "(",
    symbol_rparen: $ => ")",
    symbol_lbrace: $ => "{",
    symbol_rbrace: $ => "}",
    symbol_arrow: $ => "->",

    def: $ => seq(
      $.keyword_def,
      $.identifier,
      optional(choice($.manual_signature, $.doc_signature)),
      $.expression,
    ),

    positive_int: $ => /[1-9][0-9_]*/,
    negative_int: $ => /-[1-9][0-9_]*/,
    int: $ => choice(
      $.positive_int,
      $.negative_int,
    ),

    grouping: $ => seq(
      $.symbol_lbracket,
      repeat($.expression),
      $.symbol_rbracket,
    ),

    object: $ => seq(
      $.symbol_lbrace,
      repeat(
        $.identifier,
        // TODO types
      ),
      $.symbol_rbrace,
    ),

    type_constraint: $ => seq(
      $.symbol_lparen,
      $.expression,
      $.symbol_rparen,
    ),

    dup: $ => "dup",
    swap: $ => "swap",
    add: $ => "+",
    sub: $ => "-",
    mul: $ => "*",
    div: $ => "/",
    eq: $ => "==",
    ne: $ => "!=",
    gt: $ => ">",
    lt: $ => "<",
    gte: $ => ">=",
    lte: $ => "<=",
    and: $ => "and",
    or: $ => "or",
    not: $ => "not",
    print: $ => "print",
    malloc: $ => seq(
      $.type_constraint,
      $.keyword_malloc,
    ),
    builtin: $ => choice(
      $.dup,
      $.swap,
      $.add,
      $.sub,
      $.mul,
      $.div,
      $.eq,
      $.ne,
      $.gt,
      $.lt,
      $.gte,
      $.lte,
      $.and,
      $.or,
      $.not,
      $.print,
      $.malloc,
    ),

    local_bind: $ => seq(">$", $.identifier),
    local_var: $ => seq("$", $.identifier),
    prop_bind: $ => seq(">.", $.identifier),
    prop_var: $ => seq(".", $.identifier),

    if: $ => prec.left(1, seq(
      $.keyword_if, $.expression,
      optional(seq($.keyword_else, $.expression)),
    )),

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    expression: $ => choice(
      $.int,
      $.grouping,
      $.object,
      $.builtin,
      $.local_bind,
      $.local_var,
      $.prop_bind,
      $.prop_var,
      $.if,
      $.identifier,
    ),

    manual_signature: $ => seq(":", $.signature),
    doc_signature: $ => seq("?", $.signature),
    signature: $ => seq(
      $.symbol_lparen,
      repeat($.type),
      $.symbol_arrow,
      repeat($.type),
      $.symbol_rparen,
    ),

    type: $ => choice(
      "Int",
      "Char",
    ),
  },
  extras: $ => [
    /\s/,
  ],
});
