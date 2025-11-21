# vAPE - Interactive 4D volume visualization

[![PyPI](https://img.shields.io/pypi/v/vape4d.svg)](https://pypi.org/project/vape4d/)
[![Docs](https://img.shields.io/badge/docs-mkdocs-blue)
](https://keksboter.github.io/vape4d/docs/)

[👉 Click to run the web app 👈](https://keksboter.github.io/vape4d/?file=https://huggingface.co/datasets/vollautomat/vape4d/resolve/main/gray_scott_3d.npy&colormap=https://huggingface.co/datasets/vollautomat/vape4d/resolve/main/colormap.json)

![Viewer screenshot](https://raw.githubusercontent.com/KeKsBoTer/vape4d/master/screenshot.png)

[Burgers](https://en.wikipedia.org/wiki/Burgers%27_equation)            |  [Kuramoto–Sivashinsky](https://en.wikipedia.org/wiki/Kuramoto%E2%80%93Sivashinsky_equation)                              |  [Gray-Scott](https://groups.csail.mit.edu/mac/projects/amorphous/GrayScott/)
:-------------------------:|:------------------------:|:-------------------------:
![Burgers](https://raw.githubusercontent.com/KeKsBoTer/vape4d/master/references_3d/burgers_1.webp)  |![Kuramoto–Sivashinsky](https://raw.githubusercontent.com/KeKsBoTer/vape4d/master/references_3d/ks.webp)|![Gray Scott](https://raw.githubusercontent.com/KeKsBoTer/vape4d/master/references_3d/gray_scott.webp)|
-----


## Installation

with pip
```
pip install vape4d
```
or, alternatively, using conda
```
conda install conda-forge::vape4d
```


## Usage

**Viewer** (jupyter notebook)
```python
from vape4d import viewer
import numpy as np
from matplotlib import pyplot as plt

viewer(
    #[T,C,D,W,H]
    np.random.rand(1,1,32,32,32),
    plt.get_cmap("viridis"),   
)
```

**Render Image**
```python
import numpy as np
from vape4d import diverging_alpha, render
import matplotlib.pyplot as plt

colormap = diverging_alpha(plt.get_cmap("magma"))
img = render(
        # [T,D,H,W]
        np.random.rand(2,32,32,32).astype(np.float32),
        colormap,
        0.5, # timestep
        width=1024,
        height=1024,
    )

plt.imshow(img)
plt.axis("off")

plt.savefig("test.png", bbox_inches="tight", pad_inches=0)
plt.show()
```

## Citation

This package was developed as part of the [APEBench paper
(arxiv.org/abs/2411.00180)](https://arxiv.org/abs/2411.00180) (accepted at
NeurIPS 2024). If you find it useful for your research, please consider citing
it:

```bibtex
@article{koehler2024apebench,
  title={{APEBench}: A Benchmark for Autoregressive Neural Emulators of {PDE}s},
  author={Felix Koehler and Simon Niedermayr and R{\"}udiger Westermann and Nils Thuerey},
  journal={Advances in Neural Information Processing Systems (NeurIPS)},
  volume={38},
  year={2024}
}
```

(Feel free to also give the project a star on GitHub if you like it.)
