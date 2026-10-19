# Las 22 fórmulas
integral gaussiana:
$$\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}$$
teorema fundamental del cálculo:
$$\frac{d}{dx}\int_a^x f(t)\,dt = f(x)$$
serie de Taylor:
$$f(x) = \sum_{n=0}^{\infty} \frac{f^{(n)}(a)}{n!}(x-a)^n$$
problema de Basilea:
$$\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}$$
La límite, $\lim_{x \to 0} \frac{\sin x}{x} = 1$, en un párrafo.
determinante 3×3:
$$\det\begin{pmatrix} a & b & c \\ d & e & f \\ g & h & i \end{pmatrix} = a(ei - fh) - b(di - fg) + c(dh - eg)$$
matriz m×n:
$$A = \begin{bmatrix} a_{11} & \cdots & a_{1n} \\ \vdots & \ddots & \vdots \\ a_{m1} & \cdots & a_{mn} \end{bmatrix}$$
aligned:
$$\begin{aligned} (a+b)^2 &= (a+b)(a+b) \\ &= a^2 + 2ab + b^2 \end{aligned}$$
por casos:
$$|x| = \begin{cases} x & \text{si } x \ge 0 \\ -x & \text{si } x < 0 \end{cases}$$
La conjuntos con mathbb, $\mathbb{N} \subset \mathbb{Z} \subset \mathbb{Q} \subset \mathbb{R} \subset \mathbb{C}$, en un párrafo.
épsilon-delta:
$$\forall \varepsilon > 0\ \exists \delta > 0 : |x - a| < \delta \implies |f(x) - f(a)| < \varepsilon$$
La identidad de Euler, $e^{i\pi} + 1 = 0$, en un párrafo.
teorema de la divergencia:
$$\iiint_V (\nabla \cdot \mathbf{F})\,dV = \oiint_{\partial V} \mathbf{F} \cdot d\mathbf{S}$$
teorema de Stokes:
$$\oint_{\partial S} \mathbf{F} \cdot d\mathbf{r} = \iint_S (\nabla \times \mathbf{F}) \cdot d\mathbf{S}$$
binomio de Newton:
$$(x+y)^n = \sum_{k=0}^{n} \binom{n}{k} x^{n-k} y^k$$
transformada de Fourier:
$$\hat{f}(\xi) = \int_{\mathbb{R}} f(x)\, e^{-2\pi i x \xi}\,dx$$
densidad normal:
$$f(x) = \frac{1}{\sigma\sqrt{2\pi}} \exp\left( -\frac{(x-\mu)^2}{2\sigma^2} \right)$$
Cauchy-Schwarz:
$$\left| \langle u, v \rangle \right|^2 \le \langle u, u \rangle \cdot \langle v, v \rangle$$
suma con underbrace:
$$\underbrace{1 + 2 + \cdots + n}_{n \text{ términos}} = \frac{n(n+1)}{2}$$
ecuación del calor:
$$\frac{\partial u}{\partial t} = \alpha \nabla^2 u$$
diagrama conmutativo (amscd):
$$\begin{CD} A @>f>> B \\ @VgVV @VVhV \\ C @>k>> D \end{CD}$$
diagrama conmutativo (array):
$$\begin{array}{ccc} A & \xrightarrow{f} & B \\ \downarrow{\scriptstyle g} & & \downarrow{\scriptstyle h} \\ C & \xrightarrow{k} & D \end{array}$$
Una mal escrita, $\frac{1}{$, y un bloque de varias líneas:
$$
\begin{pmatrix} 1 & 0 \\
0 & 1 \end{pmatrix}
$$
