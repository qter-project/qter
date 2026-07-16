#import "../book.typ": book-page

#show: book-page.with(title: "Theory Introduction")

The Overview section explained how it's possible in principle to turn a Rubik's cube into a computer, however there a big 
#html.a(href: "https://github.com/PieceNotFound", style: "text-decoration: none; color: var(--sl-color-text);")[missing piece]. How do we actually find those sequences of moves that can encode "add 1" operations for registers? For this, we created an algorithm called the _Qter Architecture Solver_ (QAS) to find those special sequences.

The first half of the Theory section discusses the background knowledge and mathematical foundations required to understand the QAS, and the second half discusses how it actually works.
