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

    def: $ => seq(
      "def",
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
      "[",
      repeat($.expression),
      "]",
    ),

    object: $ => seq(
      "{",
      repeat(
        $.identifier,
        // TODO types
      ),
      "}",
    ),

    type_constraint: $ => seq(
      "(",
      $.expression,
      ")",
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
      "malloc",
    ),
    with: $ => seq(
      $.type_constraint,
      "with",
      $.expression,
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
      $.with,
    ),

    local_bind: $ => seq(">$", $.identifier),
    local_var: $ => seq("$", $.identifier),
    obj_bind: $ => seq(">@", $.identifier),
    obj_var: $ => seq("@", $.identifier),

    if: $ => prec.left(1, seq(
      "if", $.expression,
      optional(seq("else", $.expression)),
    )),

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    expression: $ => choice(
      $.int,
      $.grouping,
      $.object,
      $.builtin,
      $.local_bind,
      $.local_var,
      $.obj_bind,
      $.obj_var,
      $.if,
      $.identifier,
    ),

    manual_signature: $ => seq(":", $.signature),
    doc_signature: $ => seq("?", $.signature),
    signature: $ => seq(
      "(",
      repeat($.type),
      "->",
      repeat($.type),
      ")",
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
