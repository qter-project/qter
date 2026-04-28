#import "../book.typ": book-page, diagram

#show: book-page.with(title: "QAT Standard Library")

Lucky for you, you get a lot of macros built into the language! The QAT standard library is defined at #link("https://github.com/qter-project/qter/blob/main/src/qter_core/prelude.qat")[src/qter_core/prelude.qat] and you can reference it if you like.

```janet
sub <register> <number>
```
Subtract a number from a register

```janet
inc <register>
```
Increment a register

```janet
dec <register>
```
Decrement a register

```janet
move <register1> to <register2>
```
Zero out the first register and add its contents to the second register

```janet
set <register1> to <register2>
```
Set the contents of the first register to the contents of the second while zeroing out the contents of the second

```janet
set <register> to <number>
```
Set the contents of the first register to the number specified

```janet
if solved <register> <{}> [else <{}>]
```
Execute the code block if the register is zero, otherwise execute the `else` block if supplied

```janet
if not-solved <register> <{}> [else <{}>]
```
Execute the code block if the register is _not_ zero, otherwise execute the `else` block if supplied

```janet
if equals <register> <number> <{}> [else <{}>]
```
Execute the code block if the register equals the number supplied, otherwise execute the `else` block if supplied

```janet
if not-equals <register> <number> <{}> [else <{}>]
```
Execute the code block if the register does not equal the number supplied, otherwise execute the `else` block if supplied

```janet
if equals <register1> <register2> using <register3> <{}> [else <{}>]
```
Execute the code block if the first two registers are equal, while passing in a third register to use as bookkeeping that will be set to zero. Otherwise executes the `else` block if supplied. All three registers must have equal order. This is validated at compile-time. The equality check is implemented by decrementing both registers until one of them is zero, so the bookkeeping register is used to save the amount of times decremented.

```janet
if not-equals <register1> <register2> using <register3> <{}> [else <{}>]
```
Execute the code block if the first two registers are _not_ equal, while passing in a third register to use as bookkeeping that will be set to zero. Otherwise executes the `else` block if supplied. All three registers must have equal order. This is validated at compile-time. The equality check is implemented identically to `if equals ... using ...`.

```janet
loop <{}>
!continue
!break
```
Executes a code block in a loop forever until the `break` label or a label outside of the block is jumped to. The `break` label will exit the loop and the `continue` label will jump back to the beginning of the code block

```janet
while solved <register> <{}>
!continue
!break
```
Execute the block in a loop while the register is zero

```janet
while not-solved <register> <{}>
!continue
!break
```
Execute the block in a loop while the register is _not_ zero

```janet
while equals <register> <number> <{}>
!continue
!break
```
Execute the block in a loop while the register is equal to the number provided

```janet
while not-equals <register> <number> <{}>
!continue
!break
```
Execute the block in a loop while the register is _not_ equal to the number provided

```janet
while equals <register1> <register2> using <register3> <{}>
!continue
!break
```
Execute the block in a loop while the two registers are equal, using a third register for bookkeeping that will be zeroed out at the start of each iteration.

```janet
while not-equals <register1> <register2> using <register3> <{}>
!continue
!break
```
Execute the block in a loop while the two registers are _not_ equal, using a third register for bookkeeping that will be zeroed out at the start of each iteration.

/*
TODO: Actually implement these if we feel like it

```janet
repeat <number> [<ident>] <{}>
```
Repeat the code block the number of times supplied, optionally providing a loop index with the name specified. The index will be emitted as a `.define` statement.

```janet
repeat <ident> from <number1> to <number2> <{}>
```
Repeat the code block for each number in the range [number1, number2)

```janet
multiply <register1> <number> at <register2>
```
Add the result of multiplying the first register with the number provided to the second register, while zeroing out the first register

```janet
multiply <register1> <register2> using <register3>
```
Multiply the first two registers, storing the result in the first register and zeroing out the second, while using the third register for bookkeeping. The third register will be zeroed out. All three registers must be the same order, which is checked at compile time.
*/
