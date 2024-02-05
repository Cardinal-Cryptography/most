const gulp = require("gulp");
const shell = require("gulp-shell");

gulp.task("compile", shell.task("npm run compile"));

gulp.task("watch", function () {
  gulp.watch("contracts/*/*.rs", gulp.series("compile"));
});

gulp.task("default", gulp.series("watch"));
