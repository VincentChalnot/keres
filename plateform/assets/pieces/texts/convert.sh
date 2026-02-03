for file in *.png; do
  vtracer --path_precision=0 --mode=spline --corner_threshold=4 --splice_threshold=15 --input "$file" --output "${file%.png}.svg"
done
