package cmd

import (
	"fmt"
	"io/fs"
	"log"
	"os"
	"path/filepath"
	"strings"

	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(putCmd)

	cmnflags.AddSshClientFlags(putCmd.Flags())
	putCmd.Flags().IntP("max-workers", "w", 16, "nmber of parallel workers")
	putCmd.Flags().BoolP("recursive", "r", false, "if the copy should be recursive")

}

func putFileRecursive(sftpConn *sshc.SftpClient, remote, local string, maxWorkers int) error {
	sftpConn.ReadyWait()

	remotePath, err := sftpConn.Client.RealPath(remote)
	if err != nil {
		return fmt.Errorf("invalid remote path: %s", remotePath)
	}

	localStat, err := os.Stat(local)
	if err != nil {
		return fmt.Errorf("cannot stat local path: %s", local)
	}
	if !localStat.IsDir() {
		return fmt.Errorf("local path is not a directory: %s", local)
	}

	remoteStat, err := sftpConn.Client.Stat(remotePath)
	if err != nil {
		return fmt.Errorf("cannot stat remote path: %s", remotePath)
	}
	if !remoteStat.IsDir() {
		return fmt.Errorf("local path is not a directory: %s", remotePath)
	}

	dir := filepath.Base(local)
	err = filepath.WalkDir(local, func(localPath string, d fs.DirEntry, err error) error {
		part := strings.TrimPrefix(localPath, local)
		targetPath := filepath.Join(remotePath, dir, part)
		if d.IsDir() {
			err := sftpConn.Client.Mkdir(targetPath)
			if err != nil {
				return fmt.Errorf("cannot create directory %s: %s", remotePath, err)
			}
		} else {
			sftpConn.PutFile(targetPath, localPath, maxWorkers)
		}
		return nil
	})
	if err != nil {
		log.Println(err)
	}
	return nil
}

var putCmd = &cobra.Command{
	Use:   "put [user@]host[:port] local [remote]",
	Short: "Puts files from local to remote",
	Long:  `Puts files from local to remote`,
	Example: `
  # uploads a file to the remote server
  $ rospo put myserver:2222 ~/mylocalfolder/myfile.txt /home/myuser/

  # uploads recursively all contents of mylocalfolder to remote current working directory
  $ rospo put myserver:2222 ~/mylocalfolder -r

  # uploads recursively all contents of mylocalfolder to remote target directory
  $ rospo put myserver:2222 ~/mylocalfolder /home/myuser/myremotefolder -r
	`,
	Args: cobra.MinimumNArgs(2),
	Run: func(cmd *cobra.Command, args []string) {
		local := args[1]
		remote := ""
		if len(args) > 2 {
			remote = args[2]
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
			err := putFileRecursive(sftpConn, remote, local, maxWorkers)
			if err != nil {
				log.Printf("error while copying file: %s", err)
			}
		} else {
			err := sftpConn.PutFile(remote, local, maxWorkers)
			if err != nil {
				log.Printf("error while copying file: %s", err)
			}
		}
	},
}
