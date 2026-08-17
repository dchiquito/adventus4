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

    def: $ => seq("def", $.identifier, $.expression),

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

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

    add: $ => "+",
    sub: $ => "-",
    mul: $ => "*",
    div: $ => "/",
    print: $ => "print",
    builtin: $ => choice(
      $.add,
      $.sub,
      $.mul,
      $.div,
      $.print,
    ),

    expression: $ => choice(
      $.identifier,
      $.int,
      $.grouping,
      $.builtin,
    ),
  },
});
