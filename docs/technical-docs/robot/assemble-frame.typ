#import "../../book.typ": book-page

#show: book-page.with(title: "Assembling the Frame")

To assist with teamwork, steps are presented with dependency steps listed in parentheses; tasks can be performed in any topological order.

Note that different numbered steps should be treated as different, and the dependencies tell you which parts to use from previous steps when given a choice, since parts can have various other parts added.




= Assembling frame

== mBOM

- Structure
    - 4× 200mm Aluminum Extrusions
    - 7× 370mm Aluminum Extrusions
    - 1× 340mm Aluminum Extrusions
    - 6× Motor mounts
    - 4× Rubber feet
- Fasteners
    - 23× Brackets with corresponding t-nuts and screws
        - 1× Corresponding allen wrench
    - 7× T-plates
    - Box of m4 screws
        - 1× m4 allen wrench
    - Box of m4 t-nuts
    - Box of m3 screws
        - 1× m3 allen wrench
- Mirrors
    - 2× hexagonal mirror mounts
    - 1× top mirror
    - 1× side mirror
- Spacers
    - 1× 110mm spacer

== Routing

- Prerequisites
    - Possess the CAD model and all parts
    - Construct everything on a flat and stable surface
    - Make all of the attachments as square as possible

- Step $A$
    - Use three 370mm extrusions to construct a C shape in the pattern of the front, right, and back base extrusions in the model. Use two brackets to connect them together.
- Step $B$
    - Screw a motor mount onto the center of the 340mm extrusion.
    - Attach six brackets on the ends of the 340mm extrusion
        - Use CAD for reference; this will be the D face motor
        - Attach 
- Step $D$ ($A$, $B$)
    - Slide the 340mm extrusion into the C shape as shown in the CAD model, ensuring that the motor is to the _right_ of the extrusion
    - Use a 370mm extrusion to enclose the C shape using two brackets as shown in the CAD model; tighten the brackets on that extrusion
    - Slide the 370mm extrusion to 110mm away from the left extrusion using the 110mm spacer
    - Slide one t-nut into the ends of each of the left, back and front rails, for the two hexagonal mirrors and phone holder respectively.
- Step $E$ ($D$)
    - Take four T-plates and use ??? size screws to attach the rubber feet to the top _left_ end of the T for two of them and top _right_ for the other two
    - Attach them to the four corners of the base of the frame.
- Step $F_1, F_2$
    - Attach motor mounts to two 200mm extrusions, close to an endpoint
- Step $L$
    - Attach the top mirror to a 200mm extrusion
- Step $H$ ($F_1$, $D$)
    - Attach the extrusion from $F_1$ to the extrusion in the front using two (more) brackets. This will hold the F face motor. Ensure it is to the right of the extrusion.
- Step $I_1, I_2, I_3$
    - Attach motor mounts to three 370mm extrusions, roughly in the middle.
- Step $K$ ($I_3$)
    - Attach the side mirror to the extrusion on the same side that the motor is mounted, as shown in the CAD model
- Step $M_n, n in {1 (L), 2 (F_1), 3 (F_2)}$ ($I_n$)
    - $M_2$ only: Slide an extra bracket in the 200mm extrusion on the right side
    - Construct three L shaped joins of 370mm and 200mm extrusions as shown in the model.
- Step $N$ ($D$, $M_2$)
    - Attach the 370mm extrusion on the back side as shown in the CAD using brackets on the front and right sides. Including one on the left would prevent the back-left mirror mount from fitting.
- Step $O$ ($N$)
    - Attach the back-left hexagonal mirror mount
- Step $P$ ($M_1$, $N$)
    - Attach the L-shape with the U motor on the left side. The 200mm extrusion attaches to the 200mm extrusion from $N$. This should use three more brackets.
- Step $Q$ ($P$)
    - Attach the camera mount as shown in the model
- Step $R$ ($M_3$, $P$)
    - Remove a t-nut from one bracket
    - Slide the t-nut down the right rail
    - Slide the bracket on the right side of the 370mm 
    - Slide the 200mm extrusion into the bracket on the top-back 200mm extrusion
    - Screw the t-nut back into the bracket
- Step $S$ ($R$)
    - Attach the back-right hexagonal mirror mount

= Aligning the motors

== mBOM

- 6× Nema 17 motors
- 6× Non-wobbly couplers
- 6× 100mm Shafts
- 6× Grippers
- 1× 2.2in Rubik's cube that fits grippers

== Routing

= Final setup

== mBOM

- 1× Phone
    - 1× Charger
- 1× Phone mount
- 2× hexagonal mirrors
- Lots of rubber bands
- Lots of cable clips for 3030 extrusion

== Routing
