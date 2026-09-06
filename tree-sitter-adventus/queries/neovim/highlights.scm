(int) @number
(character) @character

(local_bind) @variable
(local_var) @variable
(prop_bind (identifier) @variable.member)
(prop_var (identifier) @variable.member)

(expression (identifier) @function.call)
(def (identifier) @function)
(builtin) @function.builtin

(keyword_def) @keyword.function
(keyword_if) @keyword.conditional
(keyword_else) @keyword.conditional
(keyword_loop) @keyword.repeat
(keyword_malloc) @keyword
(symbol_lbracket) @punctuation.bracket
(symbol_rbracket) @punctuation.bracket
(symbol_lparen) @punctuation.bracket
(symbol_rparen) @punctuation.bracket
(symbol_lbrace) @punctuation.bracket
(symbol_rbrace) @punctuation.bracket
(symbol_arrow) @punctuation.delimiter

(code_comment) @comment
(doc_comment) @comment.documentation
