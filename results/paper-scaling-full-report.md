# Paper Scaling Benchmark Report

This report is generated from `results/paper-scaling-full.json` by the committed analyzer. It reports finite local measurements, not an asymptotic runtime theorem.

## Protocol

- Fit time variable: `process_wall_time_ns`.
- Predeclared fit exclusion: `target_size < fit.minimum_target_size; missing or invalid values are excluded`.
- Timeout policy: censored and retained; excluded from exact-time fits.
- Bootstrap: seed `20260804`, `10,000` resamples.
- A slope is emitted only after `6` valid size levels satisfy the predeclared rule.
- `M` is the compressed network node count plus compressed network arc count; `K` is the explicit conflict-edge count.

## Coverage

| Family | Size range | Instances | Planned | Observed | Success | Paired | Mismatches | Timeouts | Unsupported | Errors |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| random-connected | [8, 135] | 184 | 736 | 736 | 691 | 184 | 0 | 0 | 45 | 0 |
| dense-conflict | [8, 135] | 184 | 736 | 736 | 552 | 184 | 0 | 0 | 184 | 0 |
| sparse-conflict | [8, 135] | 184 | 736 | 736 | 552 | 184 | 0 | 0 | 184 | 0 |
| comb-staircase | [8, 135] | 184 | 736 | 736 | 552 | 184 | 0 | 0 | 184 | 0 |
| supported-holes | [8, 135] | 184 | 736 | 736 | 552 | 184 | 0 | 0 | 184 | 0 |
| polyomino | [8, 135] | 184 | 736 | 736 | 552 | 184 | 0 | 0 | 184 | 0 |
| representation-crossover | [8, 135] | 184 | 736 | 736 | 552 | 184 | 0 | 0 | 184 | 0 |

## Paired timing ratios

| Family | Paired | Median compact/explicit | Bootstrap 95% CI | Stable crossover target |
| --- | --- | --- | --- | --- |
| random-connected | 184 | 1 | [1.0001320543469958, 1.0058095470936985] | none |
| dense-conflict | 184 | 1.01 | [0.9979047384983625, 1.0166970859115139] | none |
| sparse-conflict | 184 | 1 | [0.9957182841147929, 1.010875846501687] | none |
| comb-staircase | 184 | 0.993 | [0.9890381846491341, 1.0026373608647692] | none |
| supported-holes | 184 | 1 | [0.9957756436304249, 1.005867397445598] | none |
| polyomino | 184 | 1.01 | [1.0004386106759555, 1.0109932000107573] | none |
| representation-crossover | 184 | 0.996 | [0.9913661241174102, 1.0006855610028014] | 60 |

## Empirical scaling fits

| Family | Algorithm | Variable | alpha (OLS) | 95% CI | Theil-Sen | R2 | Sizes | Fit range |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| random-connected | compact-mrd | target_size | 0.0443 | [0.027035090728517137, 0.06077427855909987] | 0.0444 | 0.798 | 6 | [18, 135] |
| random-connected | compact-mrd | N | 0.0443 | [0.027035090728517137, 0.06077427855909987] | 0.0444 | 0.798 | 6 | [18, 135] |
| random-connected | compact-mrd | B | 0.0837 | [0.049041002066906626, 0.11695329512118939] | 0.074 | 0.833 | 6 | [18, 135] |
| random-connected | compact-mrd | q | 0.0692 | [0.037615399631423414, 0.09833941100381606] | 0.0648 | 0.723 | 6 | [18, 135] |
| random-connected | compact-mrd | K | 0.0348 | [0.012769472139217463, 0.05500567402826938] | 0.0331 | 0.517 | 6 | [18, 135] |
| random-connected | compact-mrd | M | 0.058 | [0.02688512867275528, 0.08595867512116515] | 0.0565 | 0.698 | 6 | [18, 135] |
| random-connected | explicit-hopcroft-karp | target_size | 0.0521 | [0.03005666671803328, 0.06606034419902722] | 0.0384 | 0.653 | 6 | [18, 135] |
| random-connected | explicit-hopcroft-karp | N | 0.0521 | [0.03005666671803328, 0.06606034419902722] | 0.0384 | 0.653 | 6 | [18, 135] |
| random-connected | explicit-hopcroft-karp | B | 0.102 | [0.054989114501222965, 0.12818062787021464] | 0.0827 | 0.736 | 6 | [18, 135] |
| random-connected | explicit-hopcroft-karp | q | 0.0863 | [0.046099372394488705, 0.10941359972816042] | 0.0617 | 0.667 | 6 | [18, 135] |
| random-connected | explicit-hopcroft-karp | K | 0.0506 | [0.019691996455563302, 0.06159401688224991] | 0.0531 | 0.647 | 6 | [18, 135] |
| random-connected | explicit-hopcroft-karp | M | 0.0774 | [0.03671897627805324, 0.09586263892713574] | 0.0887 | 0.737 | 6 | [18, 135] |
| random-connected | explicit-c0-flow | target_size | 0.0513 | [0.033730695949475864, 0.0654472078313238] | 0.0441 | 0.715 | 6 | [18, 135] |
| random-connected | explicit-c0-flow | N | 0.0513 | [0.033730695949475864, 0.0654472078313238] | 0.0441 | 0.715 | 6 | [18, 135] |
| random-connected | explicit-c0-flow | B | 0.101 | [0.06375676193851483, 0.126792663234077] | 0.0809 | 0.808 | 6 | [18, 135] |
| random-connected | explicit-c0-flow | q | 0.0879 | [0.054573176245708274, 0.11016986457876898] | 0.071 | 0.781 | 6 | [18, 135] |
| random-connected | explicit-c0-flow | K | 0.05 | [0.024812520696585044, 0.0614671808367077] | 0.0513 | 0.713 | 6 | [18, 135] |
| random-connected | explicit-c0-flow | M | 0.0682 | [0.0364643462696012, 0.08403641231206868] | 0.0677 | 0.784 | 6 | [18, 135] |
| dense-conflict | compact-mrd | target_size | 0.0878 | [0.07356760517274855, 0.09815543663663512] | 0.0681 | 0.784 | 6 | [18, 135] |
| dense-conflict | compact-mrd | N | 0.0485 | [0.040673333605622794, 0.05411031516900145] | 0.0391 | 0.802 | 6 | [18, 135] |
| dense-conflict | compact-mrd | B | 0.0896 | [0.07502725093715831, 0.10009129451982582] | 0.0723 | 0.789 | 6 | [18, 135] |
| dense-conflict | compact-mrd | q | 0.0884 | [0.07405150353618692, 0.09881700165936919] | 0.0714 | 0.787 | 6 | [18, 135] |
| dense-conflict | compact-mrd | K | 0.0464 | [0.038851168738949116, 0.051771725506792286] | 0.0374 | 0.795 | 6 | [18, 135] |
| dense-conflict | compact-mrd | M | 0.0865 | [0.07243420872731295, 0.09675610176023836] | 0.0737 | 0.783 | 6 | [18, 135] |
| dense-conflict | explicit-hopcroft-karp | target_size | 0.0799 | [0.0693528624525757, 0.0957112540390891] | 0.0538 | 0.724 | 6 | [18, 135] |
| dense-conflict | explicit-hopcroft-karp | N | 0.0441 | [0.038363863695142775, 0.05287027104798778] | 0.03 | 0.742 | 6 | [18, 135] |
| dense-conflict | explicit-hopcroft-karp | B | 0.0815 | [0.07079996045216169, 0.09765641611298263] | 0.0561 | 0.729 | 6 | [18, 135] |
| dense-conflict | explicit-hopcroft-karp | q | 0.0805 | [0.06989078984361152, 0.09640110013244371] | 0.0555 | 0.727 | 6 | [18, 135] |
| dense-conflict | explicit-hopcroft-karp | K | 0.0422 | [0.03663952989143202, 0.05053398251171037] | 0.0289 | 0.735 | 6 | [18, 135] |
| dense-conflict | explicit-hopcroft-karp | M | 0.0789 | [0.06858199916323077, 0.09451232651841734] | 0.056 | 0.727 | 6 | [18, 135] |
| dense-conflict | explicit-c0-flow | target_size | 0.163 | [0.14911968255140878, 0.17685493858992327] | 0.12 | 0.836 | 6 | [18, 135] |
| dense-conflict | explicit-c0-flow | N | 0.0896 | [0.08216685020189854, 0.09735450873994547] | 0.0687 | 0.852 | 6 | [18, 135] |
| dense-conflict | explicit-c0-flow | B | 0.166 | [0.15193503582266346, 0.18016712015927586] | 0.127 | 0.84 | 6 | [18, 135] |
| dense-conflict | explicit-c0-flow | q | 0.164 | [0.1500558890158307, 0.1779116312144704] | 0.125 | 0.838 | 6 | [18, 135] |
| dense-conflict | explicit-c0-flow | K | 0.0857 | [0.07861369531504994, 0.09316588625427814] | 0.0657 | 0.845 | 6 | [18, 135] |
| dense-conflict | explicit-c0-flow | M | 0.0881 | [0.08077700492212568, 0.09570768452516087] | 0.0676 | 0.849 | 6 | [18, 135] |
| sparse-conflict | compact-mrd | target_size | -0.0276 | [-0.03912059077788312, -0.01599467267459242] | -0.0396 | 0.596 | 6 | [18, 135] |
| sparse-conflict | compact-mrd | N | -0.0279 | [-0.039602248124100826, -0.016149890639260697] | -0.0402 | 0.593 | 6 | [18, 135] |
| sparse-conflict | compact-mrd | B | -0.0281 | [-0.03987859358309127, -0.016235587127602622] | -0.0405 | 0.591 | 6 | [18, 135] |
| sparse-conflict | compact-mrd | q | -0.0265 | [-0.03755129655549094, -0.015514647747409304] | -0.0379 | 0.607 | 6 | [18, 135] |
| sparse-conflict | compact-mrd | M | -0.0271 | [-0.038353750905445266, -0.01578563328114312] | -0.0388 | 0.601 | 6 | [18, 135] |
| sparse-conflict | explicit-hopcroft-karp | target_size | -0.0284 | [-0.03822087490619392, -0.012280202884687928] | -0.0223 | 0.711 | 6 | [18, 135] |
| sparse-conflict | explicit-hopcroft-karp | N | -0.0288 | [-0.03872581865502713, -0.012396732865293004] | -0.0226 | 0.708 | 6 | [18, 135] |
| sparse-conflict | explicit-hopcroft-karp | B | -0.029 | [-0.03902640369659225, -0.012451135980178519] | -0.0228 | 0.707 | 6 | [18, 135] |
| sparse-conflict | explicit-hopcroft-karp | q | -0.0273 | [-0.03653656803094492, -0.01193090400682284] | -0.0211 | 0.721 | 6 | [18, 135] |
| sparse-conflict | explicit-hopcroft-karp | M | -0.0278 | [-0.03738217554761352, -0.012119287161899922] | -0.0217 | 0.716 | 6 | [18, 135] |
| sparse-conflict | explicit-c0-flow | target_size | -0.019 | [-0.030848158033042878, -0.0037076208625703272] | -0.0246 | 0.445 | 6 | [18, 135] |
| sparse-conflict | explicit-c0-flow | N | -0.0192 | [-0.03126296383636139, -0.003677672441871004] | -0.0249 | 0.441 | 6 | [18, 135] |
| sparse-conflict | explicit-c0-flow | B | -0.0193 | [-0.03151323765445038, -0.0036672619721588875] | -0.0251 | 0.439 | 6 | [18, 135] |
| sparse-conflict | explicit-c0-flow | q | -0.0183 | [-0.029517357519368313, -0.0038077787488941944] | -0.0235 | 0.457 | 6 | [18, 135] |
| sparse-conflict | explicit-c0-flow | M | -0.0187 | [-0.030179592458948697, -0.0037814367340369263] | -0.024 | 0.45 | 6 | [18, 135] |
| comb-staircase | compact-mrd | target_size | 0.11 | [0.09593263102016439, 0.11977660611301633] | 0.105 | 0.509 | 6 | [18, 135] |
| comb-staircase | compact-mrd | N | 0.065 | [0.056160193433112585, 0.06997157007491055] | 0.0646 | 0.65 | 6 | [18, 135] |
| comb-staircase | compact-mrd | B | 0.132 | [0.11342912466146442, 0.14232319423378784] | 0.121 | 0.725 | 6 | [18, 135] |
| comb-staircase | explicit-hopcroft-karp | target_size | 0.0987 | [0.0893372500290035, 0.11063515367629277] | 0.121 | 0.539 | 6 | [18, 135] |
| comb-staircase | explicit-hopcroft-karp | N | 0.0584 | [0.05350435416507192, 0.06476181393416715] | 0.035 | 0.696 | 6 | [18, 135] |
| comb-staircase | explicit-hopcroft-karp | B | 0.119 | [0.10957706246565856, 0.13156101161065506] | 0.102 | 0.781 | 6 | [18, 135] |
| comb-staircase | explicit-c0-flow | target_size | 0.106 | [0.09811212306180656, 0.11816762162683703] | 0.125 | 0.582 | 6 | [18, 135] |
| comb-staircase | explicit-c0-flow | N | 0.0614 | [0.057068682236744696, 0.0683198702370747] | 0.0573 | 0.718 | 6 | [18, 135] |
| comb-staircase | explicit-c0-flow | B | 0.123 | [0.11483636258119327, 0.13762837391525573] | 0.102 | 0.78 | 6 | [18, 135] |
| supported-holes | compact-mrd | target_size | 0.0175 | [0.006616442082089493, 0.02857506258602504] | 0.0173 | 0.692 | 6 | [18, 135] |
| supported-holes | compact-mrd | N | 0.0187 | [0.007074355245933015, 0.03050085061174371] | 0.0184 | 0.7 | 6 | [18, 135] |
| supported-holes | compact-mrd | B | 0.0179 | [0.006790715890044944, 0.02929923299224664] | 0.0177 | 0.695 | 6 | [18, 135] |
| supported-holes | compact-mrd | q | 0.017 | [0.00647023854303043, 0.0278015269822915] | 0.0169 | 0.688 | 6 | [18, 135] |
| supported-holes | compact-mrd | M | 0.0173 | [0.006545546237147236, 0.02819921091006736] | 0.0171 | 0.69 | 6 | [18, 135] |
| supported-holes | explicit-hopcroft-karp | target_size | 0.023 | [0.00830337639464091, 0.0354848180782922] | 0.0234 | 0.709 | 6 | [18, 135] |
| supported-holes | explicit-hopcroft-karp | N | 0.0246 | [0.008898663422863034, 0.038016313165155566] | 0.0243 | 0.718 | 6 | [18, 135] |
| supported-holes | explicit-hopcroft-karp | B | 0.0236 | [0.008529288810750723, 0.036455593230080456] | 0.0237 | 0.713 | 6 | [18, 135] |
| supported-holes | explicit-hopcroft-karp | q | 0.0224 | [0.00808764212270063, 0.03454448134051255] | 0.0231 | 0.706 | 6 | [18, 135] |
| supported-holes | explicit-hopcroft-karp | M | 0.0227 | [0.008190656655836341, 0.035017016548233325] | 0.0232 | 0.708 | 6 | [18, 135] |
| supported-holes | explicit-c0-flow | target_size | 0.0164 | [0.008069219955274414, 0.027576526366564186] | 0.0175 | 0.657 | 6 | [18, 135] |
| supported-holes | explicit-c0-flow | N | 0.0175 | [0.008715752427185097, 0.029449512429155753] | 0.0182 | 0.663 | 6 | [18, 135] |
| supported-holes | explicit-c0-flow | B | 0.0168 | [0.008314279505580807, 0.028277235425365903] | 0.0178 | 0.659 | 6 | [18, 135] |
| supported-holes | explicit-c0-flow | q | 0.016 | [0.007831995728576709, 0.026883325576224557] | 0.0173 | 0.654 | 6 | [18, 135] |
| supported-holes | explicit-c0-flow | M | 0.0162 | [0.007945625073077299, 0.02723130668515586] | 0.0174 | 0.655 | 6 | [18, 135] |
| polyomino | compact-mrd | target_size | 0.465 | [0.4538481355753986, 0.4771967767105263] | 0.41 | 0.85 | 6 | [18, 135] |
| polyomino | compact-mrd | N | 0.25 | [0.24403409499639225, 0.2565808795682329] | 0.218 | 0.862 | 6 | [18, 135] |
| polyomino | compact-mrd | B | 0.5 | [0.48822383600685737, 0.5133245781755489] | 0.436 | 0.862 | 6 | [18, 135] |
| polyomino | explicit-hopcroft-karp | target_size | 0.462 | [0.45061497145162693, 0.4747122274932354] | 0.401 | 0.841 | 6 | [18, 135] |
| polyomino | explicit-hopcroft-karp | N | 0.249 | [0.24227150550996887, 0.25517984275428895] | 0.213 | 0.853 | 6 | [18, 135] |
| polyomino | explicit-hopcroft-karp | B | 0.498 | [0.48469669010322647, 0.5105217626951342] | 0.427 | 0.853 | 6 | [18, 135] |
| polyomino | explicit-c0-flow | target_size | 0.47 | [0.45599179870161266, 0.48110834140257697] | 0.419 | 0.862 | 6 | [18, 135] |
| polyomino | explicit-c0-flow | N | 0.253 | [0.24507020547983444, 0.2586300388202352] | 0.223 | 0.874 | 6 | [18, 135] |
| polyomino | explicit-c0-flow | B | 0.505 | [0.49029463069481444, 0.5174233925928178] | 0.446 | 0.874 | 6 | [18, 135] |
| representation-crossover | compact-mrd | target_size | 1.14 | [1.1197184318733664, 1.1505296070526903] | 1.14 | 0.951 | 6 | [18, 135] |
| representation-crossover | compact-mrd | N | 0.6 | [0.588465083742712, 0.6047060007205468] | 0.592 | 0.957 | 6 | [18, 135] |
| representation-crossover | compact-mrd | B | 1.15 | [1.1266056794775892, 1.157609853469818] | 1.14 | 0.952 | 6 | [18, 135] |
| representation-crossover | compact-mrd | q | 1.14 | [1.1197184318733664, 1.1505296070526903] | 1.14 | 0.951 | 6 | [18, 135] |
| representation-crossover | compact-mrd | K | 0.57 | [0.5598592159366832, 0.5752648035263451] | 0.568 | 0.951 | 6 | [18, 135] |
| representation-crossover | compact-mrd | M | 1.15 | [1.126605679477589, 1.157609853469818] | 1.14 | 0.952 | 6 | [18, 135] |
| representation-crossover | explicit-hopcroft-karp | target_size | 1.15 | [1.1249648214536376, 1.1542368907363434] | 1.14 | 0.951 | 6 | [18, 135] |
| representation-crossover | explicit-hopcroft-karp | N | 0.602 | [0.591223745892154, 0.6066552148893496] | 0.6 | 0.956 | 6 | [18, 135] |
| representation-crossover | explicit-hopcroft-karp | B | 1.15 | [1.131878866281734, 1.1613494669207711] | 1.15 | 0.951 | 6 | [18, 135] |
| representation-crossover | explicit-hopcroft-karp | q | 1.15 | [1.1249648214536376, 1.1542368907363434] | 1.14 | 0.951 | 6 | [18, 135] |
| representation-crossover | explicit-hopcroft-karp | K | 0.573 | [0.5624824107268188, 0.5771184453681717] | 0.572 | 0.951 | 6 | [18, 135] |
| representation-crossover | explicit-hopcroft-karp | M | 1.15 | [1.1318788662817338, 1.1613494669207711] | 1.15 | 0.951 | 6 | [18, 135] |
| representation-crossover | explicit-c0-flow | target_size | 1.17 | [1.154329112654049, 1.181464072848896] | 1.18 | 0.953 | 6 | [18, 135] |
| representation-crossover | explicit-c0-flow | N | 0.617 | [0.60663389780483, 0.6209293773393125] | 0.616 | 0.959 | 6 | [18, 135] |
| representation-crossover | explicit-c0-flow | B | 1.18 | [1.161414296348217, 1.1887275521782676] | 1.18 | 0.954 | 6 | [18, 135] |
| representation-crossover | explicit-c0-flow | q | 1.17 | [1.154329112654049, 1.181464072848896] | 1.18 | 0.953 | 6 | [18, 135] |
| representation-crossover | explicit-c0-flow | K | 0.587 | [0.5771645563270245, 0.590732036424448] | 0.589 | 0.953 | 6 | [18, 135] |
| representation-crossover | explicit-c0-flow | M | 0.591 | [0.5819086621490163, 0.595595370619722] | 0.593 | 0.954 | 6 | [18, 135] |

## Phase decomposition

The phase rows expose geometry, representation, flow/matching, recovery, and verification costs. Missing phases are not zero-cost claims; they are not applicable to that solver path.

## Interpretation boundary

A fitted slope is an empirical exponent over the declared fit interval and independent variable. It is not the exponent of the algorithm and cannot establish the unproved AN19/source-flow runtime claim. Exact-cover rows are a separate correctness Oracle category.
