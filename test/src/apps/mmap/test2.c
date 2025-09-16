#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <string.h>
#include <sys/wait.h>
#include <fcntl.h>
#include <errno.h>

#define TARGET_SIZE 4100 // 目标总长度 > 4096
#define STR_LEN 100 // 每个参数字符串的长度

int main()
{
	pid_t pid = fork();

	if (pid == -1) {
		perror("fork失败");
		exit(EXIT_FAILURE);
	}

	if (pid == 0) { // 子进程
		// 计算需要的参数数量
		int ptr_size = sizeof(char *);
		int num_args =
			(TARGET_SIZE - ptr_size) / (STR_LEN + ptr_size) + 1;

		// 创建参数数组
		char **argv = malloc((num_args + 2) * sizeof(char *));
		argv[0] = "./test1";

		// 填充长参数
		for (int i = 1; i <= num_args; i++) {
			argv[i] = malloc(STR_LEN + 1);
			memset(argv[i], 'A' + (i % 26),
			       STR_LEN); // 填充不同字符
			argv[i][STR_LEN] = '\0';
		}
		argv[num_args + 1] = NULL;

		printf("子进程 %d 执行命令: ./test1 (参数总大小 ~%ld字节)\n",
		       getpid(),
		       (long)(num_args * (STR_LEN + ptr_size) + ptr_size));

		// 执行命令
		execvp("./test1", argv);

		// 如果执行到这里说明exec失败
		perror("execvp失败");
		exit(EXIT_FAILURE);
	} else { // 父进程
		int status;
		// 等待子进程开始执行
		sleep(3); // 10ms延迟确保子进程已执行exec

		// 构建/proc/pid/cmdline路径
		char proc_path[64];
		snprintf(proc_path, sizeof(proc_path), "/proc/%d/cmdline", pid);

		// 读取cmdline内容
		int fd = open(proc_path, O_RDONLY);
		if (fd == -1) {
			perror("无法打开cmdline文件");
			waitpid(pid, &status, 0);
			return EXIT_FAILURE;
		}

		// 读取并打印cmdline
		printf("\n父进程读取 %s:\n", proc_path);
		char buffer[8192];
		ssize_t bytes_read;

		while ((bytes_read = read(fd, buffer, sizeof(buffer) - 1)) >
		       0) {
			// 将空字符替换为可见字符以便阅读
			for (int i = 0; i < bytes_read; i++) {
				if (buffer[i] == '\0')
					buffer[i] = ' ';
			}
			buffer[bytes_read] = '\0';
			printf("%s", buffer);
		}
		printf("\n");
		close(fd);

		// 等待子进程结束
		waitpid(pid, &status, 0);
		printf("子进程退出状态: %d\n", WEXITSTATUS(status));
	}

	return EXIT_SUCCESS;
}