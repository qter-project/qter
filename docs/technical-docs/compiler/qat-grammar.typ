#import "../../book.typ": book-page

#show: book-page.with(title: "QAT Grammar")

This document describes the formal grammar for the QAT language.

= Tokenizer

Tokenization occurs at the same time that parsing does; the tokenizer is driven forward by parsing. All UTF-8 characters are supported. The tokenizer first splits the text on these symbols

```
<,>  ::= ","        // Comma
<←>  ::= "<-" | "←" // Assign arrow
<⇒>  ::= "=>" | "⇒" // Define arrow
<{>  ::= "{"        // Open Brace
<}>  ::= "}"        // Close Brace
<(>  ::= "("        // Open Paren
<)>  ::= ")"        // Close Paren
<:>  ::= ":"        // Colon
<">  ::= "\""       // Quote
<\n> ::= "\n"       // New line
 _   ::= " " | "\t" // Whitespace
       | "\r"
```

Special cases:
- Whitespace does not produce tokens
- All text between quotes is kept as-is and not split.

After splitting the text, we are left with a sequence of UTF-8 strings which are tokenized in the following way:

```
.("str") ::= "." <char>*     // Directive
$<const> ::= "$" <char>*     // Constant
<num>    ::= <digit>*        // Number 
<ident>  ::= <char>*         // Ident
           | <"> <char>* <">
```

Ambiguities are resolved in the following way:
- Check for prefixes
- Test parseability as an unsigned integer
    - If parsable, it's a number
- Otherwise it's an ident

A shebang at the start of the file is special-cased and thrown away by the tokenizer.

Special commands:
- `<rhai-code>` Tells the tokenizer to dump all text byte-for-byte until seeing the text `.end-rhai`
- `<!ws>` Tells the tokenizer to assert that there is no whitespace or new-lines after a token

= Parser

Here's the grammar in BNF form. 

For readability, I'll be extending BNF notation with `*`, `+`, and `?` which have the same meaning as in regex. Also, the grammar will generally treat `<\n>` as whitespace, so I'll add a fake `<!\n>` token to notate when a newline is explicitly required to not be present. 

```
<program>       ::= <registers>? <statement>*


<registers>     ::= .("registers") <{> <register-decl>+ <}>
<register-decl> ::= (<ident> <,>)* <ident> <←> <architecture> <\n>

<architecture>  ::= <theoretical>
                  | <real>

<theoretical>   ::= "theoretical" <num>

<real>          ::= <inline>
                  | <builtin>
<inline>        ::= <ident> <algorithm!nl>
                  | <ident> <(> (<algorithm> <,>)* <algorithm> <,>? <)> 
<builtin>       ::= "builtin" <ident> <algorithm!nl>
                  | "builtin" <ident> <(> (<num> <,>)* <num> <,>? <)>

<algorithm>     ::= <ident>+
<algorithm!nl>  ::= (ident <!\n>)+


<statement>     ::= <macro>
                  | <import>
                  | <rhai_block>
                  | <instruction>
<import>        ::= .("import") <!\n> <ident> <\n>
<rhai_block>    ::= .("start-rhai") <rhai-code>

<macro>         ::= .("macro") <ident> <{> (<macro-branch>)* <macro-branch> <}>
<macro-branch>  ::= <(> (<ident> | <const> <!ws> <:> <!ws> <macro-arg-ty>)* <)> <⇒> <instruction>
<macro-arg-ty>  ::= "int" | "reg" | "block" | "ident"


<instruction>   ::= <label>
                  | <code>
                  | <const> <\n>
                  | <rhai-call>
                  | <define>
                  | <block>

<value>         ::= <const>
                  | <ident>
                  | <num>
                  | <block>

<label>         ::= <ident> <!ws> <:> <\n>
<code>          ::= <ident> <!\n> (<value> <!\n>)* <\n>
<rhai-call>     ::= "rhai" <!\n> <ident> <!\n> <(> (<value> <,>)* <value>? <,>? <)> <\n>
<define>        ::= .("define") <!\n> <ident> <!\n> <value> <\n>
                  | .("define") <!\n> <ident> <!\n> <rhai-call> <\n>
<block>         ::= <{> <instruction>* <}>
```
