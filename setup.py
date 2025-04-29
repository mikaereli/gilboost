from setuptools import setup
from setuptools_rust import Binding, RustExtension

setup(
    name="gilboost",
    version="0.1.0",
    rust_extensions=[RustExtension("gilboost", binding=Binding.PyO3)],
    zip_safe=False,
    python_requires="=3.12.0",
)