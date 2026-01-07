#import "../../book.typ": book-page

#show: book-page.with(title: "Assembling the Frame")

To assist with teamwork, steps are presented with dependency steps listed in parentheses; tasks can be performed in any topological order.

Note that different numbered steps should be treated as different, and the dependencies tell you which parts to use from previous steps when given a choice, since parts can have various other parts added.

Make all of the attachments as square as possible

- Step Zero
    - Possess the CAD model and all parts
    - Construct everything on a flat and stable surface
- Step $A$ (Zero)
    - Use three 370mm extrusions to construct a C shape in the pattern of the bottom, right, and top base extrusions in the model. Use two brackets to connect them together.
- Step $B$ (Zero)
    - Use the 340mm alignment string to screw a motor mount onto the 340mm extrusion at exactly center.
        - The edge of the mount must be flush with the flat surface. The side with the holes will not be flush.
        - Align the outer knots with the extrusion; align the motor mount with the inner knots
    - Attach six brackets to the 340mm extrusion
        - Use CAD for reference; this extrusion is the one inside the base square
        - Three on each end; the other face of the mount is flush with the end of the extrusion
            - One on the same side as the motor mount is mounted
            - One on the side facing the same direction as the motor mount (up)
            - One opposide to the first
- Step $C$ ($B$)
    - Screw a motor into the motor mount from $B$
- Step $D$ ($A$, $B$)
    - Slide the 340mm extrusion into the C shape as shown in the CAD model; do not tighten anything
- Step $E$ ($D$)
    - Use a 370mm extrusion to enclose the C shape using two brackets as shown in the CAD model; tighten the brackets on that extrusion
    - Eyeball center and leave any t-nuts that constrain side-to-side loose
    - Slide one t-nut to the ends of each of the left, back and front rails, for the two hexagonal mirrors and phone holder respectively.
- Step $F_1, F_2$ (Zero)
    - Use the 340mm alignment string to attach motor mounts to two 200mm extrusions, using one end of the extrusion as a reference point for the string.
- Step $G_n, n in {1, 2}$ ($F_n$)
    - Attach a motor to the mount
- Step $H$ ($F_1$)
    - Attach the extrusion from $F_1$ to the front bar using two (more) brackets as shown in the CAD model
- Step $I_1, I_2, I_3$ (Zero)
    - Use the 340mm Alignment string to attach motor mounts to three 370mm extrusions, using one end of the extrusion as a reference point.
- Step $J_n, n in {1, 2, 3}$ ($I_n$)
    - Screw motors into the motor mounts from $I_n$
- Step $K$ ($I_3$)
    - Attach the side mirror to the extrusion on the same side that the motor is mounted, as shown in the CAD model
- Step $L$ ($F_2$)
    - Attach the top mirror to the extrusion as shown in the CAD
- Step $M_n, n in {1, 2 (F_2), 3}$ ($I_n$)
    - $M_2$ only: Slide an extra bracket in the 200mm extrusion on the same rail that the motor is attached to
    - Construct three L shaped joins of 370mm and 200mm extrusions as shown in the model, such that the 200mm extrusion is attached on the side that wasn't used as a reference point for aligning the motor mount.
- Step $N$ ($E$, $M_2$)
    - Attach the 370mm extrusion on the back side as shown in the CAD using only one extra bracket on the right side. Including one on the left would prevent the back-left mirror mount from fitting.
- Step $O$ ($N$)
    - Attach the back-left hexagonal mirror mount
- Step $P$ ($M_1$, $N$)
    - Attach the 370mm extrusion on the left side and the 200mm extrusion to the 200mm extrusion from $N$. This should use three more brackets.
- Step $Q$ ($P$)
    - Attach the camera mount as shown in the model
- Step $R$ ($M_3$, $P$)
    - Remove the t-nut from one side of one bracket
    - Slide the t-nut down the right rail
    - Slide the bracket on the same side of the 370mm extrusion as the motor mount
    - Slide the 200mm extrusion into the bracket on the top-back 200mm extrusion
    - Screw the t-nut back into the bracket
    - Align the entire FUBD slice side-to-side so that this 370mm extrusion is square
    - Tighten everything that constrains the FUBD slice side-to-side; leave this L shape unconstrained front-to-back
- Step $S$ ($R$)
    - Attach the back-right hexagonal mirror mount
- Step $T$ ($R, C, J_1$)
    - Align the left motor relative to the bottom motor
- Step $U$ ($T, J_3$)
    - Align the right motor relative to the bottom and left motors
    - Tighten the right L shape so that it can't move front-to-back
- Step $V$ ($U, J_2, G_1, G_2$)
    - Align the rest of the motors
