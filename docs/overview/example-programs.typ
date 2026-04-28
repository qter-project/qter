#import "../book.typ": book-page

#show: book-page.with(title: "Example Programs")

= Simple

#raw(block: true, read("../../src/compiler/tests/simple/simple.qat"))

= Fibonacci

#raw(block: true, read("../../src/compiler/tests/fib/fib.qat"))

= Average

#raw(block: true, read("../../src/compiler/tests/average/average.qat"))

= Multiplication

#raw(block: true, read("../../src/compiler/tests/multiply/multiply.qat"))

= Modulus

#raw(block: true, read("../../src/compiler/tests/modulus/modulus.qat"))
