"""Tool search: BM25 today, against the same stack the other paths have.

`session/tool_index.rs` builds a BM25 index over registered MCP tools and
searches it — `bm25::SearchEngineBuilder`, plus an exact-qualified-name
short-circuit, rebuilt per call. No embedding, no reranker, no Jev, in a codebase
where memory has a vector store and a reranker slot and compaction already uses
Jev.

The catalog is the real `arithma` surface (174 tools, of which 100 are modelled
here from its own grouping) plus lookalikes it deliberately contains: `add` vs
`sumArray` vs `subtract`, `convert` vs `convertAutoDetect`, `subnetCalculator` vs
`ipInSubnet` vs `vlsmSubnets`.

    python sim_tool_search.py --transport native
"""

from __future__ import annotations

import argparse
import json
import statistics
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from rich.console import Console
from rich.table import Table

from jev import primitives, transports
from sim_embedding import cached_local
from sim_matrix import COLLECTION, milvus_search, milvus_setup, real_rerank
from sim_vs_stack import bm25_rank, dense_rank, rrf

ROOT = Path(__file__).parent
OUT = ROOT / "out"
console = Console()
WORKERS = 8
SHORTLIST = 10

SELECT_QUESTION = (
    "Which tool should be called for this request? Prefer the one that does exactly this job."
)

# name -> description, modelled on the real arithma grouping
TOOLS: dict[str, str] = {
    "add": "Add two numbers together and return the sum.",
    "subtract": "Subtract the second number from the first.",
    "multiply": "Multiply two numbers.",
    "divide": "Divide the first number by the second, erroring on zero.",
    "power": "Raise a base to an exponent.",
    "modulo": "Remainder of dividing the first number by the second.",
    "abs": "Absolute value of a number.",
    "sqrt": "Square root of a non-negative number.",
    "log": "Natural logarithm of a number.",
    "log10": "Base-10 logarithm of a number.",
    "factorial": "Factorial of a non-negative integer.",
    "sin": "Sine of an angle in radians.",
    "cos": "Cosine of an angle in radians.",
    "tan": "Tangent of an angle in radians.",
    "sumArray": "Sum every element of an array of numbers.",
    "dotProduct": "Dot product of two equal-length vectors.",
    "scaleArray": "Multiply every element of an array by a scalar.",
    "magnitudeArray": "Euclidean magnitude of a vector.",
    "compoundInterest": "Future value of a principal under compound interest.",
    "loanPayment": "Periodic payment for a loan given rate and term.",
    "presentValue": "Present value of a future cash amount.",
    "futureValueAnnuity": "Future value of a stream of equal payments.",
    "returnOnInvestment": "Return on investment as a percentage.",
    "amortizationSchedule": "Full amortization schedule for a loan.",
    "derivative": "Symbolic derivative of an expression.",
    "nthDerivative": "Nth derivative of an expression.",
    "definiteIntegral": "Definite integral of an expression over an interval.",
    "tangentLine": "Equation of the tangent line at a point.",
    "convert": "Convert a value between two named units.",
    "convertAutoDetect": "Convert a value, detecting the source unit from the text.",
    "convertCookingVolume": "Convert cooking volumes such as cups to millilitres.",
    "convertCookingWeight": "Convert cooking weights such as ounces to grams.",
    "convertOvenTemperature": "Convert oven temperatures between scales and fan settings.",
    "listCategories": "List the measurement categories available.",
    "listToolCategories": "List the tool categories available.",
    "listUnits": "List the units available in a category.",
    "getConversionFactor": "Return the conversion factor between two units.",
    "explainConversion": "Explain how a conversion is performed, step by step.",
    "convertTimezone": "Convert a timestamp between two timezones.",
    "formatDateTime": "Format a timestamp in a given pattern.",
    "currentDateTime": "Current date and time in a timezone.",
    "listTimezones": "List the known timezone identifiers.",
    "dateTimeDifference": "Difference between two timestamps.",
    "calculateWithTape": "Evaluate an expression, returning a running tape.",
    "plotFunction": "Render a plot of a function.",
    "solveEquation": "Solve an equation symbolically.",
    "findRoots": "Find the roots of a function numerically.",
    "subnetCalculator": "Given a CIDR, return network, broadcast and host range.",
    "ipToBinary": "Convert an IPv4 address to binary.",
    "binaryToIp": "Convert binary back to an IPv4 address.",
    "ipToDecimal": "Convert an IPv4 address to its decimal form.",
    "decimalToIp": "Convert a decimal back to an IPv4 address.",
    "ipInSubnet": "Check whether an IP address belongs to a subnet.",
    "vlsmSubnets": "Split a network into variable-length subnets.",
    "summarizeSubnets": "Summarize a list of subnets into the smallest covering set.",
    "expandIpv6": "Expand an IPv6 address to its full form.",
    "compressIpv6": "Compress an IPv6 address to its shortest form.",
    "transferTime": "Time to transfer a payload at a given bandwidth.",
    "throughput": "Effective throughput given overhead.",
    "tcpThroughput": "TCP throughput estimate given window and RTT.",
    "ohmsLaw": "Solve Ohm's law for the missing quantity.",
    "resistorCombination": "Combine resistors in series or parallel.",
    "capacitorCombination": "Combine capacitors in series or parallel.",
    "inductorCombination": "Combine inductors in series or parallel.",
    "voltageDivider": "Output voltage of a resistive divider.",
    "currentDivider": "Branch current of a resistive divider.",
    "rcTimeConstant": "Time constant of an RC circuit.",
    "rlTimeConstant": "Time constant of an RL circuit.",
    "rlcResonance": "Resonant frequency of an RLC circuit.",
    "impedance": "Impedance of a component at a frequency.",
    "decibelConvert": "Convert between ratio and decibels.",
    "filterCutoff": "Cutoff frequency of an RC filter.",
    "ledResistor": "Series resistor for an LED.",
    "wheatstoneBridge": "Balance condition of a Wheatstone bridge.",
    "convertBase": "Convert a number between bases.",
    "twosComplement": "Two's complement representation of a signed integer.",
    "grayCode": "Convert to or from Gray code.",
    "bitwiseOp": "Apply a bitwise operation to two integers.",
    "adcResolution": "Resolution and step size of an ADC.",
    "dacOutput": "Output voltage of a DAC for a code.",
    "timer555Astable": "Component values for a 555 astable circuit.",
    "timer555Monostable": "Pulse width for a 555 monostable circuit.",
    "evaluate": "Evaluate a mathematical expression.",
    "evaluateWithVariables": "Evaluate an expression with named variables.",
    "evaluateExact": "Evaluate an expression exactly, as a rational.",
    "evaluateExactWithVariables": "Evaluate exactly with named variables.",
    "getConversionFactorUnit": "Factor for a single unit against its base.",
    "dateTimeAdd": "Add a duration to a timestamp.",
    "roundTo": "Round a number to a given number of places.",
    "clamp": "Clamp a number between bounds.",
    "lerp": "Linear interpolation between two values.",
    "mean": "Arithmetic mean of a list of numbers.",
    "median": "Median of a list of numbers.",
    "stddev": "Standard deviation of a list of numbers.",
    "percentChange": "Percentage change between two values.",
    "ratio": "Simplify a ratio.",
    "gcd": "Greatest common divisor of two integers.",
    "lcm": "Least common multiple of two integers.",
    "primeFactors": "Prime factorisation of an integer.",
    "isPrime": "Test whether an integer is prime.",
}

REQUESTS: list[dict[str, str]] = [
    {"q": "quanto é 12 mais 30?", "gold": "add"},
    {"q": "soma todos os elementos dessa lista", "gold": "sumArray"},
    {"q": "preciso do produto escalar entre dois vetores", "gold": "dotProduct"},
    {"q": "multiplica cada elemento do array por 3", "gold": "scaleArray"},
    {"q": "qual o comprimento do vetor (3,4)?", "gold": "magnitudeArray"},
    {"q": "quanto rende 1000 a 5% ao ano em 10 anos?", "gold": "compoundInterest"},
    {"q": "qual a parcela mensal desse financiamento?", "gold": "loanPayment"},
    {"q": "converte 30 polegadas para centímetros", "gold": "convert"},
    {"q": "converte isso pra polegadas, detecta a unidade", "gold": "convertAutoDetect"},
    {"q": "quanto é 2 xícaras em mililitros?", "gold": "convertCookingVolume"},
    {"q": "180 graus em forno com ventilação, quanto fica?", "gold": "convertOvenTemperature"},
    {"q": "quais unidades existem nessa categoria?", "gold": "listUnits"},
    {"q": "qual o fator de conversão de milhas para km?", "gold": "getConversionFactor"},
    {"q": "me explica como essa conversão é feita", "gold": "explainConversion"},
    {"q": "que horas são em Tóquio agora?", "gold": "currentDateTime"},
    {"q": "converte esse horário de São Paulo para UTC", "gold": "convertTimezone"},
    {"q": "qual a diferença entre esses dois timestamps?", "gold": "dateTimeDifference"},
    {"q": "deriva x^3 + 2x", "gold": "derivative"},
    {"q": "qual a integral definida disso de 0 a 1?", "gold": "definiteIntegral"},
    {"q": "acha as raízes dessa equação", "gold": "findRoots"},
    {"q": "dado 192.168.1.10/24, qual a rede e o broadcast?", "gold": "subnetCalculator"},
    {"q": "esse IP pertence a essa sub-rede?", "gold": "ipInSubnet"},
    {"q": "divide essa rede em sub-redes de tamanhos diferentes", "gold": "vlsmSubnets"},
    {"q": "resume essa lista de sub-redes no menor conjunto que cobre tudo", "gold": "summarizeSubnets"},
    {"q": "quanto tempo leva pra transferir 2GB a 100Mbps?", "gold": "transferTime"},
    {"q": "qual o throughput efetivo considerando o overhead?", "gold": "tcpThroughput"},
    {"q": "dimensiona o resistor de série pro LED", "gold": "ledResistor"},
    {"q": "qual a frequência de ressonância desse RLC?", "gold": "rlcResonance"},
    {"q": "converte 255 pra binário", "gold": "convertBase"},
    {"q": "representa -5 em complemento de dois", "gold": "twosComplement"},
    {"q": "qual o MDC de 84 e 132?", "gold": "gcd"},
    {"q": "fatora esse número em primos", "gold": "primeFactors"},
    {"q": "esse número é primo?", "gold": "isPrime"},
    {"q": "qual a variação percentual entre 40 e 62?", "gold": "percentChange"},
    {"q": "calcula o desvio padrão dessa amostra", "gold": "stddev"},
]


@dataclass
class Arm:
    label: str
    n: int = 0
    top1: int = 0
    top3: int = 0
    gold_in_pool: int = 0
    p50_ms: float = 0.0
    tokens: int = 0
    detail: list[dict[str, Any]] = field(default_factory=list)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transport", default="native")
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True)

    corpus = [{"name": n, "description": d} for n, d in TOOLS.items()]
    names = list(TOOLS)
    index = {n: TOOLS[n] for n in names}
    docs = [f"{n}: {d}" for n, d in TOOLS.items()]
    vectors = cached_local(docs)
    qvecs = cached_local([r["q"] for r in REQUESTS])
    milvus_setup(vectors, name=f"{COLLECTION}_tools")
    coll = f"{COLLECTION}_tools"

    arms = [Arm("bm25 (hoje)"), Arm("bm25 + reranker"), Arm("milvus + jev"), Arm("milvus + reranker + jev")]

    with transports.Client.build(args.transport, timeout_s=60.0) as client:
        for arm in arms:
            latencies = []
            for req, qvec in zip(REQUESTS, qvecs):
                started = time.perf_counter()
                if arm.label.startswith("bm25"):
                    pool = bm25_rank(corpus, req["q"], SHORTLIST)
                    if arm.label.endswith("reranker"):
                        order = real_rerank(req["q"], [index[n] for n in pool])
                        if order:
                            pool = [pool[i] for i in order]
                    picked = pool[0] if pool else None
                else:
                    hits = milvus_search(qvec, SHORTLIST, name=coll)
                    pool = [names[i] for i in hits]
                    if arm.label.endswith("reranker"):
                        order = real_rerank(req["q"], [index[n] for n in pool])
                        if order:
                            pool = [pool[i] for i in order]
                    response = client.ask({"request": req["q"]},
                                          {"which": primitives.choice(SELECT_QUESTION, {n: index[n] for n in pool})})
                    arm.tokens += response.input_tokens
                    picked = response.choice("which").choice
                latencies.append((time.perf_counter() - started) * 1000)
                if picked == req["gold"]:
                    arm.top1 += 1
                if picked and picked in pool[:3]:
                    arm.top3 += 1
                if req["gold"] in pool:
                    arm.gold_in_pool += 1
            arm.n = len(REQUESTS)
            arm.p50_ms = statistics.median(latencies)

    table = Table(title=f"tool search — {len(REQUESTS)} pedidos, {len(TOOLS)} tools")
    for column in ("arm", "top-1", "gold no pool", "p50 ms", "tokens"):
        table.add_column(column, justify="right" if column != "arm" else "left")
    for arm in arms:
        table.add_row(arm.label, f"{arm.top1}/{arm.n}", f"{arm.gold_in_pool}/{arm.n}", f"{arm.p50_ms:.0f}", f"{arm.tokens:,}")
    console.print(table)

    bm = next(a for a in arms if a.label == "bm25 (hoje)")
    print("\n  onde o BM25 erra (top-1 dele contra o gold):")
    for req in REQUESTS:
        hits = bm25_rank(corpus, req["q"], 3)
        if not hits or hits[0] != req["gold"]:
            print(f"    gold={req['gold']:<20} bm25_1o={hits[0] if hits else '—':<20} {req['q'][:44]}")

    raw = OUT / "results-tool-search.json"
    raw.write_text(json.dumps([asdict(a) for a in arms], ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\n  -> {raw.name}")


if __name__ == "__main__":
    main()