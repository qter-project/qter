#import "book.typ": book-page

#show: book-page.with(title: "Introduction")

TODO: Figure out where to put the content in README.md

This book is intended to comprehensively describe Qter and how we created it. It has the following chapters:

#table(
    columns: 3,
    [], [*Description*], [*Background required*],
    [*Overview*], [What Qter is and how you can play with it], [Basic programming],
    [*Theory*], [Mathematics and algorithms behind encoding computations into move sequences], [Data structures & algorithms | discrete math],
    [*Technical Documentation*], [Implementation details of our software including lots of cool optimizations], [Software engineering],
    [*Blog*], [Stories from our journey creating this!], [None],
)

Note, most of the content in *Overview* and *Theory* was originally written for our #link("https://qter.dev/paper.pdf")[technical paper].
