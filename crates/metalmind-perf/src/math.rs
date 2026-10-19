//! Fórmulas LaTeX para los benchmarks y las capturas: las 22 del spike
//! ([08](../../../docs/research/08-matematicas.md) §2) y notas armadas con
//! ellas.

use std::fmt::Write as _;

/// Una fórmula de prueba: lo de dentro de `$…$` o de `$$…$$`.
#[derive(Clone, Copy, Debug)]
pub struct Formula {
    pub name: &'static str,
    pub latex: &'static str,
    /// Se escribe como bloque `$$…$$`.
    pub display: bool,
}

const fn display(name: &'static str, latex: &'static str) -> Formula {
    Formula {
        name,
        latex,
        display: true,
    }
}

const fn inline(name: &'static str, latex: &'static str) -> Formula {
    Formula {
        name,
        latex,
        display: false,
    }
}

/// Las 22 fórmulas de licenciatura del spike, diagramas conmutativos
/// incluidos.
pub const FORMULAS: [Formula; 22] = [
    display(
        "integral gaussiana",
        r"\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}",
    ),
    display(
        "teorema fundamental del cálculo",
        r"\frac{d}{dx}\int_a^x f(t)\,dt = f(x)",
    ),
    display(
        "serie de Taylor",
        r"f(x) = \sum_{n=0}^{\infty} \frac{f^{(n)}(a)}{n!}(x-a)^n",
    ),
    display(
        "problema de Basilea",
        r"\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}",
    ),
    inline("límite", r"\lim_{x \to 0} \frac{\sin x}{x} = 1"),
    display(
        "determinante 3×3",
        r"\det\begin{pmatrix} a & b & c \\ d & e & f \\ g & h & i \end{pmatrix} = a(ei - fh) - b(di - fg) + c(dh - eg)",
    ),
    display(
        "matriz m×n",
        r"A = \begin{bmatrix} a_{11} & \cdots & a_{1n} \\ \vdots & \ddots & \vdots \\ a_{m1} & \cdots & a_{mn} \end{bmatrix}",
    ),
    display(
        "aligned",
        r"\begin{aligned} (a+b)^2 &= (a+b)(a+b) \\ &= a^2 + 2ab + b^2 \end{aligned}",
    ),
    display(
        "por casos",
        r"|x| = \begin{cases} x & \text{si } x \ge 0 \\ -x & \text{si } x < 0 \end{cases}",
    ),
    inline(
        "conjuntos con mathbb",
        r"\mathbb{N} \subset \mathbb{Z} \subset \mathbb{Q} \subset \mathbb{R} \subset \mathbb{C}",
    ),
    display(
        "épsilon-delta",
        r"\forall \varepsilon > 0\ \exists \delta > 0 : |x - a| < \delta \implies |f(x) - f(a)| < \varepsilon",
    ),
    inline("identidad de Euler", r"e^{i\pi} + 1 = 0"),
    display(
        "teorema de la divergencia",
        r"\iiint_V (\nabla \cdot \mathbf{F})\,dV = \oiint_{\partial V} \mathbf{F} \cdot d\mathbf{S}",
    ),
    display(
        "teorema de Stokes",
        r"\oint_{\partial S} \mathbf{F} \cdot d\mathbf{r} = \iint_S (\nabla \times \mathbf{F}) \cdot d\mathbf{S}",
    ),
    display(
        "binomio de Newton",
        r"(x+y)^n = \sum_{k=0}^{n} \binom{n}{k} x^{n-k} y^k",
    ),
    display(
        "transformada de Fourier",
        r"\hat{f}(\xi) = \int_{\mathbb{R}} f(x)\, e^{-2\pi i x \xi}\,dx",
    ),
    display(
        "densidad normal",
        r"f(x) = \frac{1}{\sigma\sqrt{2\pi}} \exp\left( -\frac{(x-\mu)^2}{2\sigma^2} \right)",
    ),
    display(
        "Cauchy-Schwarz",
        r"\left| \langle u, v \rangle \right|^2 \le \langle u, u \rangle \cdot \langle v, v \rangle",
    ),
    display(
        "suma con underbrace",
        r"\underbrace{1 + 2 + \cdots + n}_{n \text{ términos}} = \frac{n(n+1)}{2}",
    ),
    display(
        "ecuación del calor",
        r"\frac{\partial u}{\partial t} = \alpha \nabla^2 u",
    ),
    display(
        "diagrama conmutativo (amscd)",
        r"\begin{CD} A @>f>> B \\ @VgVV @VVhV \\ C @>k>> D \end{CD}",
    ),
    display(
        "diagrama conmutativo (array)",
        r"\begin{array}{ccc} A & \xrightarrow{f} & B \\ \downarrow{\scriptstyle g} & & \downarrow{\scriptstyle h} \\ C & \xrightarrow{k} & D \end{array}",
    ),
];

/// Las 22 fórmulas en una nota, cada una con su nombre: las de bloque en
/// su línea y las demás dentro de un párrafo; al final, una que no compone
/// y un bloque de varias líneas.
#[must_use]
pub fn formulas_note() -> String {
    let mut text = String::from("# Las 22 fórmulas\n");
    for formula in FORMULAS {
        if formula.display {
            let _ = write!(text, "{}:\n$${}$$\n", formula.name, formula.latex);
        } else {
            let _ = writeln!(
                text,
                "La {}, ${}$, en un párrafo.",
                formula.name, formula.latex
            );
        }
    }
    text.push_str("Una mal escrita, $\\frac{1}{$, y un bloque de varias líneas:\n");
    text.push_str("$$\n\\begin{pmatrix} 1 & 0 \\\\\n0 & 1 \\end{pmatrix}\n$$\n");
    text
}

/// Una nota con `count` fórmulas distintas (cada una lleva su número), entre
/// párrafos: dos de cada tres en línea, el resto en bloque, y uno de cada
/// cuatro bloques partido en varias líneas.
#[must_use]
pub fn math_note(count: usize) -> String {
    let mut text = String::from("# Fórmulas\n\n");
    for index in 0..count {
        let formula = FORMULAS[index % FORMULAS.len()];
        let latex = format!(r"{} \quad ({index})", formula.latex);
        match index % 12 {
            n if n % 3 != 2 => {
                let _ = writeln!(
                    text,
                    "En la línea {index}, ${latex}$ es la {} y sigue el texto.",
                    formula.name
                );
            }
            11 => {
                let _ = write!(text, "\n$$\n{}\n$$\n\n", latex.replace(r" \\ ", " \\\\\n"));
            }
            _ => {
                let _ = write!(text, "\n$${latex}$$\n\n");
            }
        }
    }
    text
}

/// Una sola línea con `count` `$` que abren y nunca cierran, como precios
/// (`$a $b …`): cada uno busca su cierre en el resto de la línea.
#[must_use]
pub fn dollar_runs(count: usize) -> String {
    let mut text = String::new();
    for n in 0..count {
        let _ = write!(text, "${n}a ");
    }
    text
}

#[cfg(test)]
mod tests {
    /// La nota de las capturas (`scripts/capturas-formulas.sh`) es la de
    /// [`super::formulas_note`].
    #[test]
    fn la_nota_de_las_capturas_esta_al_dia() {
        let fixture = include_str!("../fixtures/22-formulas.md");
        assert_eq!(fixture, super::formulas_note());
    }
}
