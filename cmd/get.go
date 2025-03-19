package cmd

import (
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"

	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/logger"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/spf13/cobra"
)

var getLog = logger.NewLogger("[GET ] ", logger.Magenta)

func init() {
	rootCmd.AddCommand(getCmd)

	cmnflags.AddSshClientFlags(getCmd.Flags())
	getCmd.Flags().IntP("max-workers", "w", 16, "nmber of parallel workers")
	getCmd.Flags().BoolP("recursive", "r", false, "if the copy should be recursive")
}

func getFileRecursive(sftpConn *sshc.SftpClient, remote, local string, maxWorkers int) error {
	sftpConn.ReadyWait()

	remotePath, err := sftpConn.Client.RealPath(remote)
	if err != nil {
		return fmt.Errorf("invalid remote path: %s", remotePath)
	}

	remoteStat, err := sftpConn.Client.Stat(remotePath)
	if err != nil {
		return fmt.Errorf("cannot stat remote path: %s", remotePath)
	}
	if !remoteStat.IsDir() {
		return fmt.Errorf("remote path is not a directory: %s", remotePath)
	}

	localStat, err := os.Stat(local)
	if err != nil {
		return fmt.Errorf("cannot stat local path: %s", local)
	}
	if !localStat.IsDir() {
		return fmt.Errorf("local path is not a directory: %s", local)
	}

	dir := filepath.Dir(remotePath)
	walker := sftpConn.Client.Walk(remotePath)
	for walker.Step() {
		if walker.Err() != nil {
			log.Println(walker.Err())
			continue
		}
		remotePath := walker.Path()
		stat := walker.Stat()
		part := strings.TrimPrefix(remotePath, dir)
		localPath := filepath.Join(local, part)
		if stat.IsDir() {
			err := os.Mkdir(localPath, stat.Mode())
			if err != nil {
				return fmt.Errorf("cannot create directory %s: %s", localPath, err)
			}
		} else {
			sftpConn.GetFile(remotePath, localPath, maxWorkers)
		}
	}
	return nil
}

var getCmd = &cobra.Command{
	Use:   "get [user@]host[:port] remote [local]",
	Short: "Gets a file from remote",
	Long:  "Gets a file from remote",
	Example: `
  # downloads a file from the remote server
  $ rospo get myserver:2222 file.txt .

  # downloads recursively all contents of myremotefolder to local current working directory
  $ rospo get myserver:2222 /home/myuser/myremotefolder -r

  # downloads recursively all contents of myremotefolder to local target directory
  $ rospo get myserver:2222 /home/myserver/myremotefolder ~/mylocalfolder -r
	`,
	Args: cobra.MinimumNArgs(2),
	Run: func(cmd *cobra.Command, args []string) {
		remote := args[1]
		local := ""
		if len(args) > 2 {
			local = args[2]
		}
		recursive, _ := cmd.Flags().GetBool("recursive")
		maxWorkers, _ := cmd.Flags().GetInt("max-workers")
		sshcConf := cmnflags.GetSshClientConf(cmd, args[0])
		// sshcConf.Quiet = true
		conn := sshc.NewSshConnection(sshcConf)
		go conn.Start()

		sftpConn := sshc.NewSftpClient(conn)
		go sftpConn.Start()

		if recursive {
			err := getFileRecursive(sftpConn, remote, local, maxWorkers)
			if err != nil {
				getLog.Printf("error while copying file: %s", err)
			}
		} else {
			err := sftpConn.GetFile(remote, local, maxWorkers)
			if err != nil {
				getLog.Printf("error while copying file: %s", err)
			}
		}

	},
}
