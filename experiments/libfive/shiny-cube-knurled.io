(set-bounds! [-0.6 -0.6 -0.6] [0.6 0.6 0.6])
(set-resolution! 200)
(set-quality! 4)

;; Approximate machined/knurled version of the Rhino shiny cube.
;;
;; Source toolpath logic:
;;   ../rhino-cube-plugin/src/ShinyCubeCutPaths.cs
;;
;; Rhino's first machine op cuts each non-bottom face with two sets of
;; diagonal ball-mill passes.  This CAD visualization applies the same
;; pattern to all six faces.  The cutter centers follow the same parabolic
;; face target as shiny-cube.io; the remaining material forms a
;; diamond/knurl-like scallop pattern between passes.

(define size 1.0)
(define chamfer-size 0.05)

;; Rhino's default control-point depth is 0.5.  A quadratic 3x3 patch's
;; middle control point contributes 1/4 at the patch center, so the visible
;; center sag is about 0.125.
(define rhino-control-depth 0.5)
(define depth (/ rhino-control-depth 4.0))

(define tool-diameter 0.125)
(define tool-radius (/ tool-diameter 2.0))
(define row-count 21)

(define half-size (/ size 2.0))
(define face-half (- half-size chamfer-size))
(define chamfer-limit (- size chamfer-size))
(define sqrt2 (sqrt 2.0))
(define big 1000.0)

(define (square a) (* a a))

(define (face-falloff a)
  (max 0.0 (- 1.0 (/ (square a) (square face-half)))))

(define (face-sag a b)
  (* depth (face-falloff a) (face-falloff b)))

(define (face-boundary a b)
  (- half-size (face-sag a b)))

(define (flat-chamfered-cube ax ay az)
  (max
    (- ax half-size)
    (- ay half-size)
    (- az half-size)
    (/ (- (+ ax ay) chamfer-limit) sqrt2)
    (/ (- (+ ax az) chamfer-limit) sqrt2)
    (/ (- (+ ay az) chamfer-limit) sqrt2)))

;; Rhino's DiagonalUVs picks s values from (-1, 1), excluding the exact
;; corners.  In local face coordinates [-face-half, face-half], those rows
;; become a +/- b = offset.
(define (row-offset i)
  (* 2.0 face-half
     (+ -1.0 (* i (/ 2.0 (+ row-count 1.0))))))

(define (row-distance a b negative-slope offset)
  (/ (- (if negative-slope (+ a b) (- a b)) offset) sqrt2))

(define (row-projected-a a b negative-slope offset)
  (if negative-slope
      (- a (/ (- (+ a b) offset) 2.0))
      (- a (/ (- (- a b) offset) 2.0))))

(define (row-projected-b a b negative-slope offset)
  (if negative-slope
      (- b (/ (- (+ a b) offset) 2.0))
      (+ b (/ (- (- a b) offset) 2.0))))

;; A single pass is approximated as a sphere swept along one diagonal row.
;; `n` is the outward face coordinate; `a` and `b` are the two face-local
;; coordinates.  The pass is clipped to the square curved face patch so it
;; does not carve the planar chamfers.
(define (row-cut a b n negative-slope offset)
  (let ((pa (row-projected-a a b negative-slope offset))
        (pb (row-projected-b a b negative-slope offset)))
    (max
      (- (abs pa) face-half)
      (- (abs pb) face-half)
      (- (sqrt
           (+ (square (row-distance a b negative-slope offset))
              (square (- n (+ (face-boundary pa pb) tool-radius)))))
         tool-radius))))

(define (rows-cut a b n negative-slope i)
  (if (> i row-count)
      big
      (min
        (row-cut a b n negative-slope (row-offset i))
        (rows-cut a b n negative-slope (+ i 1)))))

(define (face-cuts a b n)
  (min
    (rows-cut a b n #f 1)
    (rows-cut a b n #t 1)))

(define-shape (knurled-shiny-cube x y z)
  (let ((ax (abs x))
        (ay (abs y))
        (az (abs z)))
    (let ((stock (flat-chamfered-cube ax ay az))
          (cutters
            (min
              (face-cuts y z x)
              (face-cuts y z (- x))
              (face-cuts x z y)
              (face-cuts x z (- y))
              (face-cuts x y z)
              (face-cuts x y (- z)))))
      (max stock (- cutters)))))

knurled-shiny-cube
