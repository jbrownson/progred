(set-bounds! [-0.6 -0.6 -0.6] [0.6 0.6 0.6])
(set-resolution! 10)
(set-quality! 8)
;; Approximate libfive version of the Rhino shiny cube.
;;
;; Source geometry:
;;   ../rhino-cube-plugin/src/ShinyCubeRhino.cs
;;
;; Rhino builds six concave degree-2 NURBS face patches plus twelve
;; planar chamfers.  This script keeps the same default dimensions and
;; approximates the face patches as implicit polynomial dimples.

;; Studio preloads the libfive environment.  Uncomment this when running
;; from plain Guile instead of Studio.
;; (use-modules (libfive kernel))

(define size 1.0)
(define chamfer-size 0.05)
(define depth 0.125)

(define half-size (/ size 2.0))
(define face-half (- half-size chamfer-size))
(define chamfer-limit (- size chamfer-size))
(define sqrt2 (sqrt 2.0))

(define (square a) (* a a))

(define (face-falloff a)
  (max 0.0 (- 1.0 (/ (square a) (square face-half)))))

;; The Rhino face patch is a square face whose center is pushed inward by
;; `depth`.  This falls to zero along the inner face square, where chamfers
;; take over.  Clamp each axis separately so the face equation does not
;; reappear outside its intended patch domain.
(define (face-sag a b)
  (* depth (face-falloff a) (face-falloff b)))

(define (face-boundary a b)
  (- half-size (face-sag a b)))

(define-shape (shiny-cube x y z)
  (let ((ax (abs x))
        (ay (abs y))
        (az (abs z)))
    (max
      ;; Original cube halfspaces.
      (- ax half-size)
      (- ay half-size)
      (- az half-size)

      ;; Edge chamfers.  In each quadrant, the edge bevel lies on
      ;; |a| + |b| = size - chamfer-size.
      (/ (- (+ ax ay) chamfer-limit) sqrt2)
      (/ (- (+ ax az) chamfer-limit) sqrt2)
      (/ (- (+ ay az) chamfer-limit) sqrt2)

      ;; Concave face patches.
      (- x (face-boundary y z))
      (- (- x) (face-boundary y z))
      (- y (face-boundary x z))
      (- (- y) (face-boundary x z))
      (- z (face-boundary x y))
      (- (- z) (face-boundary x y)))))

;; Studio renders the final shape expression.
shiny-cube

;; Headless export, if running through Guile with libfive installed:
;; (shape-save-mesh shiny-cube "shiny-cube.stl" 20
;;                  '((-0.6 . 0.6) (-0.6 . 0.6) (-0.6 . 0.6)))
