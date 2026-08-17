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
    // TODO: add the actual grammar rules
    source_file: $ => "hello"
  }
});
