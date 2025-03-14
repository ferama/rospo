package cmd

import (
	"fmt"
	"io"
	"io/fs"
	"log"
	"os"
	"path/filepath"
	"strings"
	"time"

	pb "github.com/cheggaaa/pb/v3"
	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/rio"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/pkg/sftp"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(putCmd)

	cmnflags.AddSshClientFlags(putCmd.Flags())
	putCmd.Flags().BoolP("recursive", "r", false, "if the copy should be recursive")

}

func putFile(client *sftp.Client, remote, localPath string) error {
	remotePath, err := client.RealPath(remote)
	if err != nil {
		return fmt.Errorf("invalid remote path: %s", remotePath)
	}
	remoteStat, err := client.Stat(remotePath)
	if err == nil && remoteStat.IsDir() {
		remotePath = filepath.Join(remotePath, filepath.Base(localPath))
	}

	localStat, err := os.Stat(localPath)
	if err != nil {
		return fmt.Errorf("cannot stat local path: %s", localPath)
	}

	lFile, err := os.Open(localPath)
	if err != nil {
		return fmt.Errorf("cannot open local file for read: %s", err)
	}
	defer lFile.Close()

	var offset int64
	rFile, err := client.OpenFile(remotePath, os.O_WRONLY|os.O_CREATE)
	if err == nil {
		// Check if the remote file already exists and get its size
		offset, err = rFile.Seek(0, io.SeekEnd)
		if err != nil {
			return fmt.Errorf("cannot seek remote file: %s", err)
		}
		rFile.Close()
	} else {
		offset = 0
	}

	// Reopen the remote file for writing from the offset
	rFile, err = client.OpenFile(remotePath, os.O_WRONLY|os.O_CREATE)
	if err != nil {
		return fmt.Errorf("cannot open remote file for write: %s", err)
	}
	defer rFile.Close()

	// Seek the remote file to the offset
	_, err = rFile.Seek(offset, io.SeekStart)
	if err != nil {
		return fmt.Errorf("cannot seek remote file: %s", err)
	}

	// Seek the local file to the offset
	_, err = lFile.Seek(offset, io.SeekStart)
	if err != nil {
		return fmt.Errorf("cannot seek local file: %s", err)
	}

	byteswrittench := make(chan int64)
	go func() {
		tmpl := `{{string . "target" | white}} {{with string . "prefix"}}{{.}} {{end}}{{counters . | blue }} {{bar . "[" "=" (cycle . "" "" "" "" ) " " "]" }} {{percent . | blue }} {{speed . | blue }} {{rtime . "ETA %s" | blue }}{{with string . "suffix"}} {{.}}{{end}}`
		pbar := pb.ProgressBarTemplate(tmpl).Start(0)
		pbar.Set(pb.Bytes, true)
		pbar.Set(pb.SIBytesPrefix, true)

		pbar.Set("target", filepath.Base(localPath))
		pbar.SetTotal(localStat.Size())
		for w := range byteswrittench {
			pbar.Add64(w)
		}
		pbar.Finish()
	}()
	byteswrittench <- offset
	err = rio.CopyBuffer(rFile, lFile, byteswrittench)
	close(byteswrittench)

	if err != nil {
		return fmt.Errorf("error while writing remote file: %s", err)
	}
	rFile.Chmod(localStat.Mode())
	return nil
}

func retryPutFile(conn *sshc.SshConnection, remote, local string) {
	var err error = nil
	var client *sftp.Client
	for {
		if err != nil {
			time.Sleep(1 * time.Second)
		}

		client, err = sftp.NewClient(conn.Client)
		if err != nil {
			log.Printf("err while connecting: %s", err)
			continue

		}
		defer client.Close()
		if remote == "" {
			remote, err = client.Getwd()
			if err != nil {
				log.Printf("remote is empty and I can get cwd, %s", err)
				continue
			}
		}

		err = putFile(client, remote, local)
		if err != nil {
			log.Printf("error while copying file: %s", err)
			continue
		} else {
			break
		}
	}
}

func putFileRecursive(conn *sshc.SshConnection, remote, local string) error {
	client, err := sftp.NewClient(conn.Client)
	if err != nil {
		log.Printf("err while connecting: %s", err)
		return err

	}
	defer client.Close()
	remotePath, err := client.RealPath(remote)
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

	remoteStat, err := client.Stat(remotePath)
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
			err := client.Mkdir(targetPath)
			if err != nil {
				return fmt.Errorf("cannot create directory %s: %s", remotePath, err)
			}
		} else {
			retryPutFile(conn, targetPath, localPath)
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
		sshcConf := cmnflags.GetSshClientConf(cmd, args[0])
		sshcConf.Quiet = true
		conn := sshc.NewSshConnection(sshcConf)
		go conn.Start()
		conn.ReadyWait()

		if recursive {
			err := putFileRecursive(conn, remote, local)
			if err != nil {
				log.Printf("error while copying file: %s", err)
			}
		} else {
			retryPutFile(conn, remote, local)
		}

	},
}
