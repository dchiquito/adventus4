(int) @number

(local_bind) @variable
(local_var) @variable
(prop_bind) @variable
(prop_var) @variable

(expression (identifier) @function.call)
(def (identifier) @function)
(builtin) @function.builtin

(keyword_def) @keyword.function
(keyword_if) @keyword.conditional
(keyword_else) @keyword.conditional
(keyword_malloc) @keyword
(symbol_lbracket) @punctuation.bracket
(symbol_rbracket) @punctuation.bracket
(symbol_lparen) @punctuation.bracket
(symbol_rparen) @punctuation.bracket
(symbol_lbrace) @punctuation.bracket
(symbol_rbrace) @punctuation.bracket
(symbol_arrow) @punctuation.delimiter
