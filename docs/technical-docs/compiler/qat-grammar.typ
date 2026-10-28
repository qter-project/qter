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
<%>  ::= "%"        // Modulus
<->  ::= "-"        // Negative
<.>  ::= "."        // Dot
<">  ::= "\""       // Quote
<\n> ::= "\n"       // New line
       | "//"
 _   ::= " " | "\t" // Whitespace
       | "\r"
       | "/*" char* "*/"
```

Special cases:
- All text between quotes is kept as-is and not split.
- `<->` is only split off if there isn't utf8 directly before it. "-A" is split into `<-> "A"` whereas "solved-goto" is kept as one string.

After splitting the text, we are left with a sequence of UTF-8 strings which are tokenized in the following way:

```
<const-name> ::= "$" <char>*     // Constant
<num>    ::= <digit>*        // Number 
<name>    ::= <capital-char>* // Register name (all caps)
<ident>  ::= <char>*   // Ident
           | <"> <char>* <">
```

Ambiguities are resolved in the following way:
- Check for prefixes
- Test parseability as an unsigned integer
    - If parsable, it's a number
- Check the case of each character
    - All uppercase: Register name
    - Some lowercase / All mixed case: Ident

A `.start-rhai` directive is special cased and all text after it is dumped into a `<rhai-code>` token, ending at the text `.end-rhai`.

A shebang at the start of the file is special-cased and thrown away by the tokenizer.

Special commands:
- `<!ws>` Tells the tokenizer to assert that there is no whitespace or new-lines after a token

= Level Two Tokenizer

This collects tokens into a syntax tree without doing a full parse. BNF:

```
.("str")        ::= "." <char>*                 // Directive
<register>      ::= <->? <!ws> <name>           // Register
                    (<!ws> <%> <!ws> <number>)?
$<const>        ::= <->? <!ws> <const-name>     // Constant
                    (<!ws> <%> <!ws> <number>)?
<( token* )>    ::= <(> token* <)>              // Parenthesis enclosure
<{ token* }>    ::= <(> token* <)>              // Brace enclosure
```

All other tokens are kept the same, however `<name>` and `<const-name>` are removed from the token's type signature since they're always erased

= Parser

Here's the grammar in BNF form. 

Also, the grammar will generally treat `<\n>` as whitespace, so I'll add a fake `<!\n>` token to notate when a newline is explicitly required to not be present, and also `<!ws>` meaning "no whitespace". 

```
<program>       ::= <registers>? <statement>*

<registers>     ::= .("registers") <{ <register-decl>+ }>
<register-decl> ::= (<register> <,>)* <ident> <←> <architecture> <\n>

<architecture>          ::= <inline>
                  | <builtin>
<inline>        ::= <ident> <algorithm!nl>
                  | <ident> <( (<algorithm> <,>)* <algorithm> <,>? )> 
<builtin>       ::= "builtin" <ident> <algorithm!nl>
                  | "builtin" <ident> <( (<num> <,>)* <num> <,>? )>

// (Note that `U` would parse as a register)
<algorithm>     ::= (<ident> | <register>)+
<algorithm!nl>  ::= (ident <!\n>)+


<statement>     ::= <macro>
                  | <import>
                  | <rhai_block>
                  | <instruction>
<import>        ::= .("import") <!\n> <ident> <\n>
<rhai_block>    ::= .("start-rhai") <rhai-code>

<macro>         ::= .("macro") <ident> <{ (<macro-branch>)* <macro-branch> }>
<macro-branch>  ::= <( (<ident> | <const> <!ws> <:> <!ws> <macro-arg-ty>)* )> <⇒> <instruction>
<macro-arg-ty>  ::= "int" | "reg" | "block" | "ident"


<instruction>   ::= <label>
                  | <code>
                  | <const> <\n>
                  | <rhai-call>
                  | <define>
                  | <block>

<value>         ::= <const>
                  | <ident>
                  | <register>
                  | <num>
                  | <block>

<label>         ::= <ident> <!ws> <:> <\n>
<code>          ::= <ident> <!\n> (<value> <!\n>)* <\n>
<rhai-call>     ::= "rhai" <!\n> <ident> <!\n> <( (<value> <,>)* <value>? <,>? )> <\n>
<define>        ::= .("define") <!\n> <const> <!\n> <value> <\n>
                  | .("define") <!\n> <ident> <!\n> <rhai-call> <\n>
<block>         ::= <{ <instruction>* }>
```
