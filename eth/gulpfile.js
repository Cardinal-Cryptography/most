const gulp = require("gulp");
const shell = require("gulp-shell");

gulp.task("compile", shell.task("npx hardhat compile"));

gulp.task("watch", function () {
  gulp.watch("contracts/*.sol", gulp.series("compile"));
});

gulp.task("default", gulp.series("watch"));
