export const isDev = () => {
    const development = !process.env.NODE_ENV || process.env.NODE_ENV === 'development'
    return development
}

export const sleep = async (secs) => {
    return new Promise( (resolve, _) => {
        setTimeout(() => {
            resolve(true)
        }, secs * 1000)
    })
}